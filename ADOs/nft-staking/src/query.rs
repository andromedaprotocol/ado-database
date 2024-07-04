use cosmwasm_std::Deps;

use crate::{
    msg::{ConfigResponse, RewardsPerTokenResponse},
    state::{get_rewards_per_nft, CONFIG},
    ContractError,
};

pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        denom: config.denom,
        unbonding_period: config.unbonding_period,
    })
}

pub fn query_rewards_per_token(deps: Deps) -> Result<RewardsPerTokenResponse, ContractError> {
    let rewards_per_token = get_rewards_per_nft(deps.storage)?;
    Ok(RewardsPerTokenResponse {
        rewards_per_token,
    })
}
