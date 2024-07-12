use std::collections::HashSet;

use andromeda_std::{
    andr_exec, andr_instantiate, andr_query,
    common::{denom::validate_denom, Milliseconds},
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, Deps};
use cw721::Cw721ReceiveMsg;

use crate::{error::ContractError, state::AssetDetail};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    /// Reward denomiation
    pub denom: String,
    /// List of (nft address, reward per window)
    pub rewards_per_token: Vec<(String, u128)>,
    /// optional unbonding period in seconds
    pub unbonding_period: Option<u64>,
    /// optional payout window in seconds
    pub payout_window: Option<u64>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw721ReceiveMsg),
    ClaimReward {
        nft_address: String,
        token_id: String,
    },
    Unstake {
        nft_address: String,
        token_id: String,
    },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(RewardsPerTokenResponse)]
    RewardsPerToken {},
    #[returns(StakerDetailResponse)]
    StakerDetail { staker: String },
    #[returns(AssetDetailResponse)]
    AssetDetail {
        nft_address: String,
        token_id: String,
    },
}

impl InstantiateMsg {
    pub fn validate(&self, deps: Deps) -> Result<(), ContractError> {
        // Validate reward token
        validate_denom(deps, self.denom.clone())?;

        let mut tokens = HashSet::<String>::new();
        for (token, reward) in &self.rewards_per_token {
            ensure!(!tokens.contains(token), ContractError::DuplicatedToken {});
            ensure!(*reward != 0u128, ContractError::ZeroReward {});
            tokens.insert(token.to_string());
        }

        // Rewards per token data should not be empty
        ensure!(!tokens.is_empty(), ContractError::EmptyRewardsPerToken {});

        // Unbonding period should be ge than minimum unbonding period (which is 10)
        let unbonding_period = self.unbonding_period.unwrap_or(10u64);
        ensure!(
            unbonding_period >= 10,
            ContractError::InvalidUnbondingPeriod { min: 10u64 }
        );

        // Payout window should be ge than minimum payout window(1)
        let payout_window = self.payout_window.unwrap_or(1u64);
        ensure!(
            payout_window >= 1u64,
            ContractError::InvalidPayoutWindow { min: 1u64 }
        );

        Ok(())
    }
}

#[cw_serde]
pub struct ConfigResponse {
    pub denom: String,
    pub unbonding_period: Milliseconds,
    pub payout_window: Milliseconds,
}

#[cw_serde]
pub struct RewardsPerTokenResponse {
    pub rewards_per_token: Vec<(String, u128)>,
}
#[cw_serde]
pub struct StakerDetailResponse {
    pub assets: Vec<(String, String)>,
    pub pending_rewards: u128,
}
#[cw_serde]
pub struct AssetDetailResponse {
    pub asset_detail: AssetDetail,
}
