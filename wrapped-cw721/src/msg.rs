use cosmwasm_schema::{cw_serde, QueryResponses};
use cw721::Cw721ReceiveMsg;
use andromeda_std::{
    andr_instantiate, andr_exec, andr_query,
    amp::AndrAddr,
};
#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub authorized_token_addresses: Option<Vec<AndrAddr>>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    ReceiveNft(Cw721ReceiveMsg),
    SetWrappedNftAddress {
        wrapped_nft_address: AndrAddr,
    },
}

#[cw_serde]
pub enum Cw721HookMsg {
    MintWrappedNft {
        sender: AndrAddr,
        wrapped_token_owner: String,
        unwrappable: bool,
    },
    UnwrapNft {
        recipient: AndrAddr,
        wrapped_token: AndrAddr,
        wrapped_token_id: String,
    },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(OriginCw721InfoResponse)]
    GetOriginCw721 {
        wrapped_token: AndrAddr,
        wrapped_token_id: String,
    },

    #[returns(WrappedCw721InfoResponse)]
    GetWrappedCw721 {
        origin_token: AndrAddr,
        origin_token_id: String,
    },

    #[returns(bool)]
    IsUnwrappable {
        wrapped_token: AndrAddr,
        wrapped_token_id: String,
    },

    #[returns(u64)]
    GetWrappedNftCount {},

    #[returns(AndrAddr)]
    GetWrappedNftAddress {},

    #[returns(Option<Vec<AndrAddr>>)]
    GetAuthorizedTokenAddresses {},
}

#[cw_serde]
pub struct OriginCw721InfoResponse {
    pub origin_token: AndrAddr,
    pub origin_token_id: String,
}

#[cw_serde]
pub struct WrappedCw721InfoResponse {
    pub wrapped_token: AndrAddr,
    pub wrapped_token_id: String,
}
