use andromeda_std::amp::AndrAddr;
use cw_storage_plus::{Map, Item};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;

pub const WRAPPED_NFT_ADDRESS: Item<AndrAddr> = Item::new("wrapped_token_address");
pub const WRAPPED_NFT_COUNT: Item<u64> = Item::new("wrapped_token_count");
pub const AUTHORIED_TOKEN_ADDRESSES: Item<Vec<AndrAddr>> = Item::new("authorized_token_addresses");

pub const WRAPPED_INFO: Map<(&Addr, &str), WrappedInfo> = Map::new("wrapped_info");
pub const ORIGIN_INFO: Map<(&Addr, &str), OriginInfo> = Map::new("origin_info");

#[cw_serde]
pub struct WrappedInfo {
    pub wrapped_token: AndrAddr,
    pub wrapped_token_id: String,
    pub unwrappable: bool,
}

#[cw_serde]
pub struct OriginInfo {
    pub origin_token: AndrAddr,
    pub origin_token_id: String,
    pub unwrappable: bool,
}

