use andromeda_std::{
    amp::{AndrAddr, Recipient},
    andr_exec, andr_instantiate, andr_query,
    common::milliseconds::MillisecondsDuration,
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use cw721::Cw721ReceiveMsg;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub authorized_token_addresses: Option<Vec<AndrAddr>>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    ReceiveNft(Cw721ReceiveMsg),
    ClaimNft {
        cw721_contract: AndrAddr,
        token_id: String,
    },
}

#[cw_serde]
pub enum Cw721HookMsg {
    TimelockNft {
        lock_duration: MillisecondsDuration,
        recipient: Recipient,
    },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(UnlockTimeResponse)]
    UnlockTime {
        cw721_contract: AndrAddr,
        token_id: String,
    },
    #[returns(NftDetailsResponse)]
    NftDetails {
        cw721_contract: AndrAddr,
        token_id: String,
    },
    #[returns(IsLockedResponse)]
    IsLocked {
        cw721_contract: AndrAddr,
        token_id: String,
    },
}

#[cw_serde]
pub struct UnlockTimeResponse {
    pub unlock_time: u64,
}

#[cw_serde]
pub struct NftDetailsResponse {
    pub unlock_time: u64,
    pub recipient: Addr,
}

#[cw_serde]
pub struct IsLockedResponse {
    pub is_locked: bool,
}
