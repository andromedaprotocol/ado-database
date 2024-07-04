use andromeda_std::common::Milliseconds;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Storage, Uint64};
use cw_storage_plus::{Item, Map};

use crate::ContractError;

#[cw_serde]
#[derive(Default)]
pub struct Config {
    pub denom: String,
    pub unbonding_period: Milliseconds,
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const REWARDS_PER_NFT: Map<&str, Uint64> = Map::new("rewards_per_nft");

pub fn set_rewards_per_nft(
    store: &mut dyn Storage,
    rewards_per_nft: Vec<(String, u64)>,
) -> Result<(), ContractError> {
    rewards_per_nft.iter().for_each(|item| {
        REWARDS_PER_NFT
            .save(store, &item.0, &Uint64::from(item.1))
            .unwrap();
    });
    Ok(())
}
pub fn get_rewards_per_nft(store: &dyn Storage) -> Result<Vec<(String, u64)>, ContractError> {
    Ok(REWARDS_PER_NFT
        .range_raw(store, None, None, cosmwasm_std::Order::Ascending)
        .map(|item| {
            let item = item.unwrap();
            (String::from_utf8(item.0).unwrap(), item.1.u64())
        })
        .collect())
}
