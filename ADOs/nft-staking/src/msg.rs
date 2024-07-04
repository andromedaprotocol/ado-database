use std::collections::HashSet;

use andromeda_std::{
    andr_exec, andr_instantiate, andr_query,
    common::{denom::validate_denom, Milliseconds},
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, Deps};

use crate::error::ContractError;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub denom: String,
    pub rewards_per_nft: Vec<(String, u64)>,
    pub unbonding_period: Option<u64>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(RewardsPerTokenResponse)]
    RewardsPerToken {},
}

impl InstantiateMsg {
    pub fn validate(&self, deps: Deps) -> Result<(), ContractError> {
        // Validate reward token
        validate_denom(deps, self.denom.clone())?;

        let mut nfts = HashSet::<String>::new();
        for (nft, reward) in &self.rewards_per_nft {
            ensure!(!nfts.contains(nft), ContractError::DuplicatedNFT {});
            ensure!(*reward != 0u64, ContractError::ZeroReward {});
            nfts.insert(nft.to_string());
        }

        // Rewards per nft data should not be empty
        ensure!(!nfts.is_empty(), ContractError::EmptyRewardsPerNFT {});

        // Unbonding period should be non zero
        let unbonding_period = self.unbonding_period.unwrap_or(10u64);
        ensure!(
            unbonding_period >= 10,
            ContractError::InvalidUnbondingPeriod { min: 10u64 }
        );

        Ok(())
    }
}

#[cw_serde]
pub struct ConfigResponse {
    pub denom: String,
    pub unbonding_period: Milliseconds,
}

#[cw_serde]
pub struct RewardsPerTokenResponse {
    pub rewards_per_token: Vec<(String, u64)>,
}
