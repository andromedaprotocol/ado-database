use andromeda_std::common::Milliseconds;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, Addr, BlockInfo, Storage, Uint128};
use cw_storage_plus::{Item, Map};

use crate::ContractError;

#[cw_serde]
#[derive(Default)]
pub struct Config {
    pub denom: String,
    pub unbonding_period: Milliseconds,
    pub payout_window: Milliseconds,
}

#[cw_serde]
#[derive(Default)]
pub struct StakerDetail {
    // list of staked nft assets as (nft_address, nft_id)
    pub assets: Vec<(String, String)>,
}

#[cw_serde]
#[derive(Default)]
pub struct AssetDetail {
    pub nft_address: String,
    pub token_id: String,
    pub unbonding_period: Milliseconds,
    pub pending_rewards: Uint128,
    pub updated_at: Milliseconds,
    pub unstaked_at: Option<Milliseconds>,
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const REWARDS_PER_TOKEN: Map<&str, Uint128> = Map::new("rewards_per_token");

pub const STAKER_DETAILS: Map<&Addr, StakerDetail> = Map::new("staker_details");

pub const ASSET_DETAILS: Map<(String, String), AssetDetail> = Map::new("asset_details");

pub fn set_rewards_per_token(
    store: &mut dyn Storage,
    rewards_per_token: Vec<(String, u128)>,
) -> Result<(), ContractError> {
    rewards_per_token.iter().for_each(|item| {
        REWARDS_PER_TOKEN
            .save(store, &item.0, &Uint128::from(item.1))
            .unwrap();
    });
    Ok(())
}
pub fn get_rewards_per_token(store: &dyn Storage) -> Result<Vec<(String, u128)>, ContractError> {
    Ok(REWARDS_PER_TOKEN
        .range_raw(store, None, None, cosmwasm_std::Order::Ascending)
        .map(|item| {
            let item = item.unwrap();
            (String::from_utf8(item.0).unwrap(), item.1.u128())
        })
        .collect())
}

pub fn get_staker_detail(store: &dyn Storage, staker: Addr) -> Result<StakerDetail, ContractError> {
    let staker_detail = STAKER_DETAILS.load(store, &staker)?;
    Ok(staker_detail)
}

pub fn get_asset_detail(
    store: &dyn Storage,
    block: &BlockInfo,
    nft_address: String,
    token_id: String,
) -> Result<AssetDetail, ContractError> {
    let mut asset_detail = ASSET_DETAILS.load(store, (nft_address, token_id))?;
    asset_detail.pending_rewards = calculate_pending_rewards(store, block, asset_detail.clone());
    Ok(asset_detail)
}

pub fn process_pending_rewards(
    store: &mut dyn Storage,
    block: &BlockInfo,
    nft_address: String,
    token_id: String,
) -> Result<Uint128, ContractError> {
    let mut asset_detail = ASSET_DETAILS.load(store, (nft_address.clone(), token_id.clone()))?;
    let pending_rewards = calculate_pending_rewards(store, block, asset_detail.clone());
    ensure!(!pending_rewards.is_zero(), ContractError::ZeroReward {});

    let unpaid_duration = block
        .time
        .nanos()
        .checked_sub(asset_detail.updated_at.nanos())
        .unwrap_or_default();

    let config = CONFIG.load(store).unwrap_or_default();
    let remainder = unpaid_duration % config.payout_window.nanos();
    asset_detail.pending_rewards = Uint128::zero();
    asset_detail.updated_at = Milliseconds::from_nanos(block.time.nanos() - remainder);
    ASSET_DETAILS.save(store, (nft_address, token_id), &asset_detail)?;
    Ok(pending_rewards)
}

pub fn calculate_pending_rewards(
    store: &dyn Storage,
    block: &BlockInfo,
    asset_detail: AssetDetail,
) -> Uint128 {
    // For nfts in unbonding period, just return original pending rewards
    if asset_detail.unstaked_at.is_some() {
        return asset_detail.pending_rewards;
    }

    let unpaid_duration: u128 = block
        .time
        .nanos()
        .checked_sub(asset_detail.updated_at.nanos())
        .unwrap_or_default() as u128;
    let config = CONFIG.load(store).unwrap_or_default();

    let window_count = Uint128::new(unpaid_duration / config.payout_window.nanos() as u128);

    let reward_per_window = REWARDS_PER_TOKEN
        .load(store, &asset_detail.nft_address)
        .unwrap_or_default();

    window_count
        .checked_mul(reward_per_window)
        .unwrap_or_default()
        .checked_add(asset_detail.pending_rewards)
        .unwrap()
}
