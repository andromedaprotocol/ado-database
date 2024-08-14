use andromeda_std::common::milliseconds::MillisecondsExpiration;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Map;

pub const TIMELOCKS: Map<(&Addr, &str), TimelockInfo> = Map::new("timelocks");

#[cw_serde]
pub struct TimelockInfo {
    pub unlock_time: MillisecondsExpiration,
    pub recipient: Addr,
}
