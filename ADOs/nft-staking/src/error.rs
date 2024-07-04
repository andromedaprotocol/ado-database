use andromeda_std::error::ContractError as AndromedaContractError;
use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    ADO(#[from] AndromedaContractError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Empty Rewards Per NFT")]
    EmptyRewardsPerNFT {},

    #[error("Duplicated NFT")]
    DuplicatedNFT {},

    #[error("Reward should be non zero")]
    ZeroReward {},

    #[error("Unbonding Period should be longer than {min} seconds")]
    InvalidUnbondingPeriod { min: u64 },
}

impl Into<AndromedaContractError> for ContractError {
    fn into(self) -> AndromedaContractError {
        match self {
            ContractError::Std(err) => err.into(),
            _ => panic!("Unsupported error type"),
        }
    }
}
