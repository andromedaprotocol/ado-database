use cosmwasm_std::{Addr, Deps, Env, Order};

use crate::{
    msg::{
        AssetDetailResponse, ConfigResponse, RewardsPerTokenResponse, StakerDetailResponse,
        StakersResponse,
    },
    state::{get_asset_detail, get_rewards_per_token, get_staker_detail, CONFIG, STAKER_DETAILS},
    ContractError,
};

pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        denom: config.denom,
        unbonding_period: config.unbonding_period,
        payout_window: config.payout_window,
    })
}

pub fn query_rewards_per_token(deps: Deps) -> Result<RewardsPerTokenResponse, ContractError> {
    let rewards_per_token = get_rewards_per_token(deps.storage)?;
    Ok(RewardsPerTokenResponse { rewards_per_token })
}

pub fn query_stakers(deps: Deps) -> Result<StakersResponse, ContractError> {
    let stakers: Vec<String> = STAKER_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|staker| staker.unwrap().to_string())
        .collect();
    Ok(StakersResponse { stakers })
}

pub fn query_staker_detail(
    deps: Deps,
    env: Env,
    staker: String,
) -> Result<StakerDetailResponse, ContractError> {
    let staker = Addr::unchecked(staker);
    let staker_detail = get_staker_detail(deps.storage, staker)?;
    let pending_rewards = staker_detail
        .assets
        .iter()
        .map(|(nft_address, token_id)| {
            get_asset_detail(
                deps.storage,
                &env.block,
                nft_address.to_string(),
                token_id.to_string(),
            )
            .unwrap_or_default()
            .pending_rewards
            .u128()
        })
        .sum();
    Ok(StakerDetailResponse {
        assets: staker_detail.assets,
        pending_rewards,
    })
}

pub fn query_asset_detail(
    deps: Deps,
    env: Env,
    nft_address: String,
    token_id: String,
) -> Result<AssetDetailResponse, ContractError> {
    let asset_detail = get_asset_detail(deps.storage, &env.block, nft_address, token_id)?;

    Ok(AssetDetailResponse { asset_detail })
}
