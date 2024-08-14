use crate::contract::{execute, instantiate, query};
use crate::msg::{
    Cw721HookMsg::TimelockNft, ExecuteMsg, InstantiateMsg, NftDetailsResponse, QueryMsg,
    UnlockTimeResponse,
};
use andromeda_std::ado_base::ownership::ContractOwnerResponse;
use andromeda_std::testing::mock_querier::MOCK_KERNEL_CONTRACT;
use anyhow::Error;
use cosmwasm_std::{testing::mock_env, Addr, Empty};
use cw721::OwnerOfResponse;
use cw721_base::entry::{
    execute as cw721_execute, instantiate as cw721_instantiate, query as cw721_query,
};
use cw721_base::MinterResponse;
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use andromeda_std::{
    amp::{AndrAddr, Recipient},
    common::{encode_binary, milliseconds::MillisecondsDuration},
};

const ONE_DAY: u64 = 24 * 60 * 60;

fn mock_app() -> App {
    App::default()
}

pub fn contract_cw721_timelock() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn contract_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(cw721_execute, cw721_instantiate, cw721_query);
    Box::new(contract)
}

#[test]
fn cw721_timelock_test() {
    let mut router: App = mock_app();

    let addr1: Addr = Addr::unchecked("addr1");
    let addr2: Addr = Addr::unchecked("addr2");
    let origin_cw721_owner = Addr::unchecked("origin_cw721_owner");
    let recipient = Addr::unchecked("recipient");

    let cw721_timelock_id: u64 = router.store_code(contract_cw721_timelock());
    let cw721_id: u64 = router.store_code(contract_cw721());

    let cw721_timelock_instantiate_msg: InstantiateMsg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_token_addresses: None,
    };

    let cw721_instantiate_msg: cw721_base::InstantiateMsg = cw721_base::msg::InstantiateMsg {
        name: "Test Timelock CW721".to_string(),
        symbol: "TL-CW721".to_string(),
        minter: addr2.to_string(),
    };

    let cw721_timelock_addr: Addr = router
        .instantiate_contract(
            cw721_timelock_id,
            addr1.clone(),
            &cw721_timelock_instantiate_msg,
            &[],
            "CW721 Timelock",
            None,
        )
        .unwrap();

    let query_owner_msg = QueryMsg::Owner {};
    let owner_res: ContractOwnerResponse = router
        .wrap()
        .query_wasm_smart(&cw721_timelock_addr, &query_owner_msg)
        .unwrap();

    // Assert that the owner of the CW721 Timelock contract is addr1
    assert_eq!(addr1, owner_res.owner);

    let cw721_addr: Addr = router
        .instantiate_contract(
            cw721_id,
            addr2.clone(),
            &cw721_instantiate_msg,
            &[],
            "CW721",
            Some(origin_cw721_owner.to_string()),
        )
        .unwrap();

    let query_minter_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::Minter {};
    let minter_res: MinterResponse = router
        .wrap()
        .query_wasm_smart(&cw721_addr, &query_minter_msg)
        .unwrap();

    // Assert that the minter of the CW721 contract is addr2
    assert_eq!(Some(addr2.to_string()), minter_res.minter);

    let cw721_mint_msg: cw721_base::msg::ExecuteMsg<Empty, Empty> =
        cw721_base::msg::ExecuteMsg::Mint {
            token_id: "token1".to_string(),
            owner: origin_cw721_owner.to_string(),
            token_uri: None,
            extension: Empty::default(),
        };
    // Only minter of cw721 can execute mint function
    router
        .execute_contract(addr2.clone(), cw721_addr.clone(), &cw721_mint_msg, &[])
        .unwrap();

    //** Another method to execute contract **//

    // let execute_mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
    //     contract_addr: cw721_addr.to_string(),
    //     msg: encode_binary(&cw721_mint_msg).unwrap(),
    //     funds: vec![],
    // });
    // router.execute(addr2, execute_mint_msg).unwrap();

    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf {
        token_id: "token1".to_string(),
        include_expired: None,
    };
    let owner_res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(&cw721_addr, &query_owner_msg)
        .unwrap();

    // Assert that the owner of the token1 is origin_cw721_owner
    assert_eq!(origin_cw721_owner.to_string(), owner_res.owner);

    let hook_msg = TimelockNft {
        lock_duration: MillisecondsDuration::from_seconds(3 * ONE_DAY),
        recipient: Recipient::new(recipient.to_string(), None),
    };

    let send_cw721_msg: cw721_base::msg::ExecuteMsg<Empty, Empty> =
        cw721_base::msg::ExecuteMsg::SendNft {
            contract: cw721_timelock_addr.to_string(),
            token_id: "token1".to_string(),
            msg: encode_binary(&hook_msg).unwrap(),
        };

    let env = mock_env();
    let expected_unlock_time = env.block.time.seconds() + 3 * ONE_DAY;

    router
        .execute_contract(
            origin_cw721_owner.clone(),
            cw721_addr.clone(),
            &send_cw721_msg,
            &[],
        )
        .unwrap();

    let owner_res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(&cw721_addr, &query_owner_msg)
        .unwrap();

    // Assert that the owner of token1 is now the CW721 Timelock contract
    assert_eq!(cw721_timelock_addr.to_string(), owner_res.owner);

    let query_unlock_info_msg = QueryMsg::UnlockTime {
        cw721_contract: AndrAddr::from_string(cw721_addr.to_string()),
        token_id: "token1".to_string(),
    };
    let unlock_info_res: UnlockTimeResponse = router
        .wrap()
        .query_wasm_smart(&cw721_timelock_addr, &query_unlock_info_msg)
        .unwrap();

    // Assert that the unlock time is as expected
    assert_eq!(expected_unlock_time, unlock_info_res.unlock_time);

    let query_nft_details_msg = QueryMsg::NftDetails {
        cw721_contract: AndrAddr::from_string(cw721_addr.to_string()),
        token_id: "token1".to_string(),
    };
    let nft_details_res: NftDetailsResponse = router
        .wrap()
        .query_wasm_smart(&cw721_timelock_addr, &query_nft_details_msg)
        .unwrap();

    // Assert that the recipient is as set
    assert_eq!(recipient, nft_details_res.recipient);

    let execute_claim_msg = ExecuteMsg::ClaimNft {
        cw721_contract: AndrAddr::from_string(cw721_addr.to_string()),
        token_id: "token1".to_string(),
    };

    router.update_block(|block| {
        block.time = block.time.plus_seconds(2 * ONE_DAY);
    });

    // Attempt to claim before unlock time and expect an error
    let err_res: Error = router
        .execute_contract(
            origin_cw721_owner.clone(),
            cw721_timelock_addr.clone(),
            &execute_claim_msg,
            &[],
        )
        .unwrap_err();
    let err_str = "error executing WasmMsg:\nsender: origin_cw721_owner\nExecute { contract_addr: \"contract0\", msg: {\"claim_nft\":{\"cw721_contract\":\"contract1\",\"token_id\":\"token1\"}}, funds: [] }";
    assert_eq!(err_res.to_string(), err_str.to_string());

    router.update_block(|block| {
        block.time = block.time.plus_seconds(3 * ONE_DAY);
    });

    // Attempt to claim after unlock time
    router
        .execute_contract(
            origin_cw721_owner.clone(),
            cw721_timelock_addr.clone(),
            &execute_claim_msg,
            &[],
        )
        .unwrap();

    let owner_res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(&cw721_addr, &query_owner_msg)
        .unwrap();

    // Assert that the owner of token1 is now the recipient
    assert_eq!(recipient, owner_res.owner);
}
