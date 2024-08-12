#![cfg(test)]
use andromeda_std::common::Milliseconds;
use andromeda_std::testing::mock_querier::{MOCK_ADO_PUBLISHER, MOCK_KERNEL_CONTRACT};
use cosmwasm_std::{coin, Addr, Binary, BlockInfo, Empty};
use cw_multi_test::{
    App, AppBuilder, BankKeeper, Contract, ContractWrapper, Executor, MockAddressGenerator,
    MockApiBech32, WasmKeeper,
};

use crate::contract::{execute, instantiate, query};
use crate::msg::{AssetDetailResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakerDetailResponse};
const MOCK_USER: &str = "user";

type MockApp = App<BankKeeper, MockApiBech32>;

fn mock_app() -> MockApp {
    AppBuilder::new()
        .with_api(MockApiBech32::new("andr"))
        .with_wasm(WasmKeeper::new().with_address_generator(MockAddressGenerator))
        .build(|router, _api, storage| {
            router
                .bank
                .init_balance(
                    storage,
                    &Addr::unchecked("bank"),
                    [coin(100000000000000000, "uandr")].to_vec(),
                )
                .unwrap();
        })
}

pub fn contract_nft_staking() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn contract_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    );
    Box::new(contract)
}

fn generate_mock_address(input: &str) -> Addr {
    MockApiBech32::new("andr").addr_make(input)
}

#[test]
fn test_nft_staking() {
    let mut router = mock_app();

    let mock_publisher = generate_mock_address(MOCK_ADO_PUBLISHER);
    let mock_user = generate_mock_address(MOCK_USER);
    let kernel_addr = generate_mock_address(MOCK_KERNEL_CONTRACT);

    // instantiation
    let cw721_id: u64 = router.store_code(contract_cw721());
    let nft_staking_id = router.store_code(contract_nft_staking());

    let cw721_instantiate_msg = cw721_base::msg::InstantiateMsg {
        name: "Andromeda NFT".to_string(),
        symbol: "ANFT".to_string(),
        minter: mock_publisher.to_string(),
    };

    let cw721_addr = router
        .instantiate_contract(
            cw721_id,
            mock_publisher.clone(),
            &cw721_instantiate_msg,
            &[],
            "Andromeda",
            None,
        )
        .unwrap();
    let nft_staking_instantiate_msg = InstantiateMsg {
        denom: "uandr".to_string(),
        rewards_per_token: vec![(cw721_addr.to_string(), 10u128)],
        unbonding_period: Some(10u64),
        payout_window: Some(10u64),
        kernel_address: kernel_addr.to_string(),
        owner: None,
    };

    let funds = &[coin(1000000u128, "uandr")];
    router
        .send_tokens(Addr::unchecked("bank"), mock_publisher.clone(), funds)
        .unwrap();

    let nft_staking_addr = router
        .instantiate_contract(
            nft_staking_id,
            mock_publisher.clone(),
            &nft_staking_instantiate_msg,
            funds,
            "Andromeda",
            None,
        )
        .unwrap();
    router
        .send_tokens(Addr::unchecked("bank"), nft_staking_addr.clone(), funds)
        .unwrap();
    let cw721_mint_msg: cw721_base::ExecuteMsg<Empty, Empty> = cw721_base::ExecuteMsg::Mint {
        token_id: "1".to_string(),
        owner: mock_user.to_string(),
        token_uri: None,
        extension: Empty::default(),
    };

    router
        .execute_contract(mock_publisher, cw721_addr.clone(), &cw721_mint_msg, &[])
        .unwrap();

    // Stake token 1
    let cw721_transfer_msg: cw721_base::ExecuteMsg<Empty, Empty> =
        cw721_base::ExecuteMsg::SendNft {
            // contract: rand_addr.to_string(),
            contract: nft_staking_addr.to_string(),
            token_id: "1".to_string(),
            msg: Binary::default(),
        };

    router
        .execute_contract(
            mock_user.clone(),
            cw721_addr.clone(),
            &cw721_transfer_msg,
            &[],
        )
        .unwrap();

    // Check asset detail
    let query_asset_detail = QueryMsg::AssetDetail {
        nft_address: cw721_addr.to_string(),
        token_id: "1".to_string(),
    };
    let res: AssetDetailResponse = router
        .wrap()
        .query_wasm_smart(nft_staking_addr.clone(), &query_asset_detail.clone())
        .unwrap();

    assert_eq!(res.asset_detail.nft_address, cw721_addr.to_string());
    assert_eq!(res.asset_detail.token_id, "1".to_string());
    assert_eq!(
        res.asset_detail.unbonding_period,
        Milliseconds::from_seconds(10u64)
    );
    assert_eq!(
        res.asset_detail.updated_at,
        Milliseconds::from_nanos(router.block_info().time.nanos())
    );

    // Wait for 10 senconds
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: router.block_info().time.plus_seconds(10),
        chain_id: router.block_info().chain_id,
    });
    let res: AssetDetailResponse = router
        .wrap()
        .query_wasm_smart(nft_staking_addr.clone(), &query_asset_detail.clone())
        .unwrap();

    assert_eq!(res.asset_detail.pending_rewards.u128(), 10);

    let query_staker_detail = QueryMsg::StakerDetail {
        staker: mock_user.to_string(),
    };
    let res: StakerDetailResponse = router
        .wrap()
        .query_wasm_smart(nft_staking_addr.clone(), &query_staker_detail.clone())
        .unwrap();
    assert_eq!(res.pending_rewards, 10);

    // Claim Reward
    let claim_reward_msg = ExecuteMsg::ClaimReward {
        nft_address: cw721_addr.to_string(),
        token_id: "1".to_string(),
    };

    router
        .execute_contract(
            mock_user.clone(),
            nft_staking_addr.clone(),
            &claim_reward_msg,
            &[],
        )
        .unwrap();
    let user_balance = router
        .wrap()
        .query_balance(mock_user.clone(), "uandr")
        .unwrap();
    assert_eq!(user_balance.amount.u128(), 10);

    // Unstake
    let unstake_msg = ExecuteMsg::Unstake {
        nft_address: cw721_addr.to_string(),
        token_id: "1".to_string(),
    };
    router
        .execute_contract(
            mock_user.clone(),
            nft_staking_addr.clone(),
            &unstake_msg,
            &[],
        )
        .unwrap();

    // Wait for unbonding period
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: router.block_info().time.plus_seconds(11),
        chain_id: router.block_info().chain_id,
    });

    // pending_rewards should be zero for unstaked tokens
    let res: AssetDetailResponse = router
        .wrap()
        .query_wasm_smart(nft_staking_addr.clone(), &query_asset_detail.clone())
        .unwrap();
    assert_eq!(res.asset_detail.pending_rewards.u128(), 0);
    assert!(res.asset_detail.unstaked_at.is_some());

    // claim asset
    let claim_asset_msg = ExecuteMsg::ClaimAsset {
        nft_address: cw721_addr.to_string(),
        token_id: "1".to_string(),
    };
    router
        .execute_contract(
            mock_user.clone(),
            nft_staking_addr.clone(),
            &claim_asset_msg,
            &[],
        )
        .unwrap();

    // mock_user has no staked asset now
    let query_staker_detail = QueryMsg::StakerDetail {
        staker: mock_user.to_string(),
    };
    let res: StakerDetailResponse = router
        .wrap()
        .query_wasm_smart(nft_staking_addr.clone(), &query_staker_detail.clone())
        .unwrap();

    assert!(res.assets.is_empty());
    assert_eq!(res.pending_rewards, 0);
}
