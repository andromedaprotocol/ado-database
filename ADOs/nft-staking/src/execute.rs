use andromeda_std::common::{context::ExecuteContext, encode_binary, Milliseconds};
use cosmwasm_std::{coin, ensure, Addr, BankMsg, CosmosMsg, Response, Uint128, WasmMsg};
use cw721::{Cw721ExecuteMsg, Cw721ReceiveMsg};

use crate::{
    state::{
        get_asset_detail, get_staker_detail, process_pending_rewards, AssetDetail, StakerDetail,
        ASSET_DETAILS, CONFIG, REWARDS_PER_TOKEN, STAKER_DETAILS,
    },
    ContractError,
};

pub fn receive_cw721(ctx: ExecuteContext, msg: Cw721ReceiveMsg) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let nft_address = info.sender;
    let staker = msg.sender;
    let staker_addr = Addr::unchecked(staker.clone());
    let token_id = msg.token_id;

    ensure!(
        REWARDS_PER_TOKEN.has(deps.storage, &nft_address.to_string()),
        ContractError::InvalidToken {}
    );

    // add staker to the staker list
    let mut staker_detail = STAKER_DETAILS
        .load(deps.storage, &staker_addr.clone())
        .unwrap_or_default();
    staker_detail
        .assets
        .push((nft_address.to_string(), token_id.clone()));

    STAKER_DETAILS.save(deps.storage, &staker_addr.clone(), &staker_detail)?;

    // add nft to the staked asset list
    let asset_id = asset_id(nft_address.clone(), token_id.clone());
    ensure!(
        !ASSET_DETAILS.has(deps.storage, asset_id.clone()),
        ContractError::DuplicatedAsset {}
    );

    let config = CONFIG.load(deps.storage)?;
    ASSET_DETAILS.save(
        deps.storage,
        asset_id,
        &AssetDetail {
            nft_address: nft_address.to_string(),
            token_id: token_id.clone(),
            unbonding_period: config.unbonding_period,
            pending_rewards: Uint128::zero(),
            updated_at: Milliseconds::from_nanos(env.block.time.nanos()),
            unstaked_at: None,
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "receive_cw721")
        .add_attribute("nft_address", nft_address.to_string())
        .add_attribute("token_id", token_id)
        .add_attribute("staker", staker))
}

pub fn claim_reward(
    ctx: ExecuteContext,
    nft_address: String,
    token_id: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let staked_assets = get_staker_detail(deps.storage, info.sender.clone())?.assets;
    let asset_id = (nft_address.clone(), token_id.clone());

    // Ensure sender staked the asset
    ensure!(
        staked_assets.contains(&asset_id),
        ContractError::Unauthorized {}
    );

    let denom = CONFIG.load(deps.storage)?.denom;
    let pending_rewards = process_pending_rewards(
        deps.storage,
        &env.block,
        nft_address.clone(),
        token_id.clone(),
    )?;

    Ok(Response::new()
        .add_attribute("method", "claim_reward")
        .add_attribute("nft_address", nft_address.to_string())
        .add_attribute("token_id", token_id)
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(pending_rewards.u128(), denom)],
        })))
}

pub fn unstake(
    ctx: ExecuteContext,
    nft_address: String,
    token_id: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let staked_assets = get_staker_detail(deps.storage, info.sender.clone())?.assets;
    let asset_id = (nft_address.clone(), token_id.clone());

    // Ensure sender staked the asset
    ensure!(
        staked_assets.contains(&asset_id),
        ContractError::Unauthorized {}
    );

    // asset_detail with pending_rewards calculated
    let mut asset_detail = get_asset_detail(
        deps.storage,
        &env.block,
        nft_address.clone(),
        token_id.clone(),
    )?;

    // Can not unstake tokens that is already unstaked
    ensure!(
        asset_detail.unstaked_at.is_none(),
        ContractError::AssetAlreadyUnstaked {}
    );

    // Set unstaked_at and updated_at
    let curr_time = Milliseconds::from_nanos(env.block.time.nanos());
    asset_detail.updated_at = curr_time;
    asset_detail.unstaked_at = Some(curr_time);

    ASSET_DETAILS.save(deps.storage, asset_id, &asset_detail)?;

    Ok(Response::new()
        .add_attribute("method", "unstake")
        .add_attribute("nft_address", nft_address.to_string())
        .add_attribute("token_id", token_id))
}

pub fn claim_asset(
    ctx: ExecuteContext,
    nft_address: String,
    token_id: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let mut staked_assets = get_staker_detail(deps.storage, info.sender.clone())?.assets;
    let asset_id = (nft_address.clone(), token_id.clone());

    // Ensure the asset surpassed the unbonding period
    let asset_detail = get_asset_detail(
        deps.storage,
        &env.block,
        nft_address.clone(),
        token_id.clone(),
    )?;
    ensure!(
        asset_detail.unstaked_at.is_some(),
        ContractError::AssetNotUnkstaked {}
    );

    ensure!(
        staked_assets.contains(&asset_id),
        ContractError::Unauthorized {}
    );

    staked_assets.retain(|x| x.0 != nft_address.clone() || x.1 != token_id.clone());

    let claimable_at = asset_detail
        .unstaked_at
        .unwrap()
        .plus_milliseconds(asset_detail.unbonding_period);

    ensure!(
        claimable_at.is_expired(&env.block),
        ContractError::AssetOnUnbondingPeriod {}
    );

    STAKER_DETAILS.save(
        deps.storage,
        &info.sender,
        &StakerDetail {
            assets: staked_assets,
        },
    )?;
    ASSET_DETAILS.remove(deps.storage, asset_id);
    let pending_rewards = asset_detail.pending_rewards;
    let denom = CONFIG.load(deps.storage)?.denom;

    let mut resp = Response::new()
        .add_attribute("method", "claim_asset")
        .add_attribute("nft_address", nft_address.to_string())
        .add_attribute("token_id", token_id.clone())
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: nft_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: info.sender.to_string(),
                token_id: token_id,
            })?,
            funds: vec![],
        }));

    if !pending_rewards.is_zero() {
        resp = resp.add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(pending_rewards.u128(), denom)],
        }));
    }

    Ok(resp)
}

pub fn asset_id(nft_address: impl Into<String>, token_id: String) -> (String, String) {
    (nft_address.into(), token_id)
}

#[cfg(test)]
mod tests {
    use crate::{
        contract::instantiate,
        msg::InstantiateMsg,
        query::{query_asset_detail, query_config, query_staker_detail},
    };

    use super::*;
    use andromeda_std::testing::mock_querier::MOCK_ADO_PUBLISHER;
    use cosmwasm_std::{
        coin,
        testing::{
            mock_dependencies_with_balance, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
            MOCK_CONTRACT_ADDR,
        },
        Binary, Empty, OwnedDeps,
    };
    use cw721::Cw721ReceiveMsg;
    const STAKER: &str = "STAKER";

    fn inst() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);

        let rewards_per_token = vec![(MOCK_CONTRACT_ADDR.to_string(), 1u128)];
        let msg = InstantiateMsg {
            denom: "earth".to_string(),
            rewards_per_token,
            unbonding_period: Some(100u64),
            kernel_address: "kernel".to_string(),
            owner: None,
            payout_window: None,
        };
        instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap();

        deps
    }

    #[test]
    fn test_stake() {
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let mut deps = inst();

        let env = mock_env();
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());

        let res = receive_cw721(ctx, cw721_msg);
        assert!(res.is_ok());

        let res = query_staker_detail(deps.as_ref(), env.clone(), STAKER.to_string()).unwrap();

        assert_eq!(res.pending_rewards, 0);
        assert_eq!(
            res.assets,
            vec![(MOCK_CONTRACT_ADDR.to_string(), token_id.clone())]
        );

        let config = query_config(deps.as_ref()).unwrap();
        let res = query_asset_detail(
            deps.as_ref(),
            env.clone(),
            MOCK_CONTRACT_ADDR.to_string(),
            token_id.clone(),
        )
        .unwrap();
        assert_eq!(res.asset_detail.pending_rewards.u128(), 0u128);
        assert_eq!(res.asset_detail.unbonding_period, config.unbonding_period);
        assert_eq!(res.asset_detail.unstaked_at, None);

        assert_eq!(
            res.asset_detail.updated_at,
            Milliseconds::from_nanos(env.block.time.nanos())
        );
    }

    #[test]
    fn test_stake_invalid_token() {
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info("INVALID_CONTRACT", &[]);

        let mut deps = inst();

        let env = mock_env();
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());

        let err = receive_cw721(ctx, cw721_msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidToken {});
    }

    #[test]
    fn test_claim_reward() {
        let mut deps = inst();
        let mut env = mock_env();
        let config = query_config(deps.as_ref()).unwrap();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // claim reward
        let info = mock_info(STAKER, &[]);
        let window_cnt = Milliseconds::from_seconds(100).nanos() / config.payout_window.nanos();
        let updated_at = env.block.time.nanos() + window_cnt * config.payout_window.nanos();

        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let res = claim_reward(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("method", "claim_reward")
                .add_attribute("nft_address", MOCK_CONTRACT_ADDR.to_string())
                .add_attribute("token_id", token_id.clone())
                .add_message(CosmosMsg::Bank(BankMsg::Send {
                    to_address: STAKER.to_string(),
                    amount: vec![coin(window_cnt as u128 * 1u128, "earth")],
                }))
        );

        let res = query_asset_detail(deps.as_ref(), env, MOCK_CONTRACT_ADDR.to_string(), token_id)
            .unwrap();

        assert_eq!(res.asset_detail.pending_rewards.u128(), 0);
        assert_eq!(
            res.asset_detail.updated_at,
            Milliseconds::from_nanos(updated_at)
        );
    }

    #[test]
    fn test_claim_reward_unauthorized() {
        let mut deps = inst();
        let env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // Other one stake token 2
        let token_id = "2".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: "other".to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // STAKER claim other one's reward
        let info = mock_info(STAKER, &[]);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = claim_reward(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn test_claim_reward_zero_reward() {
        let mut deps = inst();
        let env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // claim reward
        let info = mock_info(STAKER, &[]);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = claim_reward(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id).unwrap_err();

        assert_eq!(err, ContractError::ZeroReward {});
    }

    #[test]
    fn test_unstake() {
        let mut deps = inst();
        let mut env = mock_env();
        let config = query_config(deps.as_ref()).unwrap();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);
        let window_cnt = Milliseconds::from_seconds(100).nanos() / config.payout_window.nanos();
        let updated_at = env.block.time.nanos() + window_cnt * config.payout_window.nanos();

        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let res = unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone());
        assert!(res.is_ok());

        let res = query_asset_detail(deps.as_ref(), env, MOCK_CONTRACT_ADDR.to_string(), token_id)
            .unwrap();

        assert_eq!(
            res.asset_detail.pending_rewards.u128(),
            window_cnt as u128 * 1u128
        );
        assert_eq!(
            res.asset_detail.updated_at,
            Milliseconds::from_nanos(updated_at)
        );
        assert_eq!(
            res.asset_detail.unstaked_at.unwrap(),
            Milliseconds::from_nanos(updated_at)
        );
    }

    #[test]
    fn test_unstake_unauthorized() {
        let mut deps = inst();
        let env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // Other one stake token 2
        let token_id = "2".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: "other".to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // STAKER unstake other one's reward
        let info = mock_info(STAKER, &[]);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn test_unstake_unstaked_token() {
        let mut deps = inst();
        let mut env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);
        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info.clone(), env.clone());
        unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap();

        // unstake token again
        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env);
        let err = unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();

        assert_eq!(err, ContractError::AssetAlreadyUnstaked {})
    }

    #[test]
    fn test_claim_asset() {
        let mut deps = inst();
        let mut env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);

        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info.clone(), env.clone());
        unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap();

        // claim asset
        let config = query_config(deps.as_ref()).unwrap();
        env.block.time = env
            .block
            .time
            .plus_seconds(config.unbonding_period.seconds() + 1);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let res = claim_asset(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone());
        assert!(res.is_ok());
    }

    #[test]
    fn test_claim_asset_not_unstaked() {
        let mut deps = inst();
        let env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = claim_asset(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();
        assert_eq!(err, ContractError::AssetNotUnkstaked {});
    }

    #[test]
    fn test_claim_unbonding_asset() {
        let mut deps = inst();
        let mut env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);

        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info.clone(), env.clone());
        unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap();

        // claim asset
        let config = query_config(deps.as_ref()).unwrap();
        env.block.time = env
            .block
            .time
            .plus_seconds(config.unbonding_period.seconds() - 1);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = claim_asset(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();
        assert_eq!(err, ContractError::AssetOnUnbondingPeriod {});
    }
    #[test]
    fn test_claim_asset_unauthorized() {
        let mut deps = inst();
        let mut env = mock_env();

        // STAKER stake token 1
        let token_id = "1".to_string();
        let cw721_msg = Cw721ReceiveMsg {
            sender: STAKER.to_string(),
            token_id: token_id.clone(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // other one stake token 2
        let cw721_msg = Cw721ReceiveMsg {
            sender: "other".to_string(),
            token_id: "2".to_string(),
            msg: Binary::default(),
        };

        let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        receive_cw721(ctx, cw721_msg).unwrap();

        // unstake
        let info = mock_info(STAKER, &[]);

        env.block.time = env.block.time.plus_seconds(100);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        unstake(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap();

        // claim asset
        let info = mock_info("other", &[]);
        let config = query_config(deps.as_ref()).unwrap();
        env.block.time = env
            .block
            .time
            .plus_seconds(config.unbonding_period.seconds() + 1);
        let ctx = ExecuteContext::new(deps.as_mut(), info, env.clone());
        let err = claim_asset(ctx, MOCK_CONTRACT_ADDR.to_string(), token_id.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }
}
