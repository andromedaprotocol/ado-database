use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg,
    ado_contract::ADOContract,
    common::{actions::call_action, context::ExecuteContext, encode_binary},
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    query,
    state::{set_rewards_per_nft, Config, CONFIG},
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:nft-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_UNBONDING_PERIOD: u64 = 10u64; // 10 seconds

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    msg.validate(deps.as_ref())?;

    let contract = ADOContract::default();

    let unbonding_period = msg.unbonding_period.unwrap_or(MIN_UNBONDING_PERIOD);

    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    CONFIG.save(
        deps.storage,
        &Config {
            denom: msg.denom,
            unbonding_period: andromeda_std::common::Milliseconds::from_seconds(unbonding_period),
        },
    )?;

    set_rewards_per_nft(deps.storage, msg.rewards_per_nft)?;

    Ok(resp
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);
    handle_execute(ctx, msg)
    // if let ExecuteMsg::AMPReceive(pkt) = msg {
    //     ADOContract::default().execute_amp_receive::<ContractError>(ctx, pkt, handle_execute).map_err(|err| err.into())
    // } else {
    // handle_execute(ctx, msg)
    // }
}

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(encode_binary(&query::query_config(deps)?)?),
        _ => Ok(ADOContract::default().query(deps, env, msg)?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andromeda_std::common::Milliseconds;
    use andromeda_std::error::ContractError as AndromedaContractError;
    use andromeda_std::testing::mock_querier::MOCK_ADO_PUBLISHER;
    use cosmwasm_std::coin;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info, MOCK_CONTRACT_ADDR,
    };

    fn mock_instantiate_msg() -> InstantiateMsg {
        let rewards_per_nft = vec![(MOCK_CONTRACT_ADDR.to_string(), 1u64)];
        InstantiateMsg {
            denom: "earth".to_string(),
            rewards_per_nft,
            unbonding_period: Some(100u64),
            kernel_address: "kernel".to_string(),
            owner: None,
        }
    }
    #[test]
    fn test_instantiate() {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);
        let msg = mock_instantiate_msg();
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        );

        assert!(res.is_ok());

        let config = query::query_config(deps.as_ref()).unwrap();
        assert_eq!(config.denom, "earth".to_string());
        assert_eq!(config.unbonding_period, Milliseconds::from_seconds(100u64));

        let res = query::query_rewards_per_token(deps.as_ref()).unwrap();
        assert_eq!(
            res.rewards_per_token,
            vec![(MOCK_CONTRACT_ADDR.to_string(), 1u64)]
        );
    }
    #[test]
    fn test_instantiate_invalid_denom() {
        let mut deps = mock_dependencies();
        let msg = mock_instantiate_msg();
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap_err();

        assert_eq!(
            err,
            ContractError::ADO(AndromedaContractError::InvalidAsset {
                asset: "earth".to_string()
            })
        );
    }
    #[test]
    fn test_instantiate_emtpy_rewards_per_nft() {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);
        let mut msg = mock_instantiate_msg();
        msg.rewards_per_nft = vec![];
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap_err();

        assert_eq!(err, ContractError::EmptyRewardsPerNFT {});
    }
    #[test]
    fn test_instantiate_duplicated_rewards_per_nft() {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);
        let mut msg = mock_instantiate_msg();
        msg.rewards_per_nft
            .push((MOCK_CONTRACT_ADDR.to_string(), 10u64));
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap_err();

        assert_eq!(err, ContractError::DuplicatedNFT {});
    }
    #[test]
    fn test_instantiate_zero_reward_per_nft() {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);
        let mut msg = mock_instantiate_msg();
        msg.rewards_per_nft = vec![(MOCK_CONTRACT_ADDR.to_string(), 0u64)];
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap_err();

        assert_eq!(err, ContractError::ZeroReward {});
    }
    #[test]
    fn test_instantiate_invalid_unbonding_period() {
        let balance = vec![coin(1000u128, "earth")];
        let mut deps = mock_dependencies_with_balance(&balance);
        let mut msg = mock_instantiate_msg();
        msg.unbonding_period = Some(5u64);
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info(MOCK_ADO_PUBLISHER, &[]),
            msg,
        )
        .unwrap_err();

        assert_eq!(err, ContractError::InvalidUnbondingPeriod { min: 10u64 });
    }
}
