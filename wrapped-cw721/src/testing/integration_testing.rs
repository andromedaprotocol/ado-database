use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, Cw721HookMsg, QueryMsg};
use cosmwasm_std::{Empty, coin, StdError, Addr};
use cw_multi_test::{App, Contract, ContractWrapper, Executor, AppBuilder, BankKeeper, MockApiBech32, WasmKeeper, MockAddressGenerator};
use cw721_base::entry::{execute as cw721_execute, instantiate as cw721_instantiate, query as cw721_query};
use cw721::OwnerOfResponse;

use andromeda_std::{
    common::encode_binary,
    amp::AndrAddr,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
    ado_base::ownership::ContractOwnerResponse,
};

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
                    [coin(10000000000000, "uandr")].to_vec(),
                )
                .unwrap();
        })
}

pub fn contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(cw721_execute, cw721_instantiate, cw721_query);
    Box::new(contract)
}

fn generate_mock_address(input: &str) -> Addr {
    MockApiBech32::new("andr").addr_make(input)
}

#[test]
fn wrapped_cw721_test() {
    let mut router = mock_app();

    let addr1 = generate_mock_address("addr1");
    let addr2 = generate_mock_address("addr2");

    let origin_nft_owner = generate_mock_address("origin_nft_owner");
    let wrapped_nft_owner = generate_mock_address("wrapped_nft_owner");

    let kernel_addr = generate_mock_address(MOCK_KERNEL_CONTRACT);

    let origin_nft_id: u64 = router.store_code(cw721());
    let origin_nft_instantiate_msg: cw721_base::InstantiateMsg = cw721_base::msg::InstantiateMsg {
        name: "Origin Nft".to_string(),
        symbol: "O-Nft".to_string(),
        minter: addr1.to_string(),
    };
    let origin_nft_addr: Addr = router.instantiate_contract(
        origin_nft_id,
        addr1.clone(),
        &origin_nft_instantiate_msg,
        &[],
        "Origin Nft",
        Some(origin_nft_owner.to_string()),
    ).unwrap();

    let contract_id: u64 = router.store_code(contract());
    let contract_instantiate_msg: InstantiateMsg = InstantiateMsg {
        owner: None,
        kernel_address: kernel_addr.to_string(),
        authorized_token_addresses: Some(vec![
            AndrAddr::from_string(origin_nft_addr.to_string()),
        ]),
    };
    let contract_addr: Addr = router.instantiate_contract(
        contract_id,
        addr1.clone(),
        &contract_instantiate_msg,
        &[],
        "Wrapped Cw721 Contract",
        None,
    ).unwrap();

    let query_owner_msg = QueryMsg::Owner {};
    let owner_res: ContractOwnerResponse = router
        .wrap()
        .query_wasm_smart(&contract_addr, &query_owner_msg)
        .unwrap();
    // Assert that the owner of the contract is addr1
    assert_eq!(addr1, owner_res.owner);

    let wrapped_nft_id: u64 = router.store_code(cw721());
    let wrapped_nft_instantiate_msg: cw721_base::InstantiateMsg = cw721_base::msg::InstantiateMsg {
        name: "Wrapped Nft".to_string(),
        symbol: "W-Nft".to_string(),
        minter: contract_addr.to_string(),
    };
    let wrapped_nft_addr: Addr = router.instantiate_contract(
        wrapped_nft_id,
        addr2.clone(),
        &wrapped_nft_instantiate_msg,
        &[],
        "Wrapped Nft",
        None,
    ).unwrap();

    router.execute_contract(
        addr1.clone(), 
        contract_addr.clone(), 
        &ExecuteMsg::SetWrappedNftAddress { wrapped_nft_address: AndrAddr::from_string(wrapped_nft_addr.to_string())}, 
        &[],
    ).unwrap();


    let origin_nft_mint_msg: cw721_base::msg::ExecuteMsg<Empty, Empty> = cw721_base::msg::ExecuteMsg::Mint { 
        token_id: "token1".to_string(), 
        owner: origin_nft_owner.to_string(), 
        token_uri: None, 
        extension: Empty::default(),
    };
    router.execute_contract(
        addr1.clone(), 
        origin_nft_addr.clone(), 
        &origin_nft_mint_msg, 
        &[],
    ).unwrap();
    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf { 
        token_id: "token1".to_string(),
        include_expired: None, 
    };
    let owner_res: OwnerOfResponse = router.wrap().query_wasm_smart(&origin_nft_addr, &query_owner_msg).unwrap();
    // Assert that the owner of the token1 is origin_cw721_owner
    assert_eq!(origin_nft_owner.to_string(), owner_res.owner);

    let hook_msg = Cw721HookMsg::MintWrappedNft { 
        sender: AndrAddr::from_string(origin_nft_addr.to_string()),
        wrapped_token_owner: wrapped_nft_owner.to_string(), 
        unwrappable: true, 
    };
    let send_origin_nft_msg: cw721_base::msg::ExecuteMsg<Empty, Empty> = cw721_base::msg::ExecuteMsg::SendNft { 
        contract: contract_addr.to_string(), 
        token_id: "token1".to_string(), 
        msg: encode_binary(&hook_msg).unwrap(),
    };
    router.execute_contract(
        origin_nft_owner.clone(), 
        origin_nft_addr.clone(), 
        &send_origin_nft_msg, 
        &[],
    ).unwrap();

    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf { 
        token_id: "token1".to_string(),
        include_expired: None, 
    };
    let owner_res: OwnerOfResponse = router.wrap().query_wasm_smart(&origin_nft_addr, &query_owner_msg).unwrap();
    // Assert that the owner of the token1 is contract_addr
    assert_eq!(contract_addr.to_string(), owner_res.owner);

    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf { 
        token_id: "wrapped_token1".to_string(),
        include_expired: None, 
    };
    let owner_res: OwnerOfResponse = router.wrap().query_wasm_smart(&wrapped_nft_addr, &query_owner_msg).unwrap();
    // Assert that the owner of the token1 is wrapped_nft_owner
    assert_eq!(wrapped_nft_owner.to_string(), owner_res.owner);


    let hook_msg = Cw721HookMsg::UnwrapNft { 
        recipient: AndrAddr::from_string(origin_nft_owner.to_string()),  
        wrapped_token: AndrAddr::from_string(wrapped_nft_addr.to_string()), 
        wrapped_token_id: "wrapped_token1".to_string(), 
    };
    let send_wrapped_nft_msg: cw721_base::msg::ExecuteMsg<Empty, Empty> = cw721_base::msg::ExecuteMsg::SendNft { 
        contract: contract_addr.to_string(), 
        token_id: "wrapped_token1".to_string(), 
        msg: encode_binary(&hook_msg).unwrap(),
    };
    router.execute_contract(
        wrapped_nft_owner.clone(), 
        wrapped_nft_addr.clone(), 
        &send_wrapped_nft_msg, 
        &[],
    ).unwrap();

    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf { 
        token_id: "token1".to_string(),
        include_expired: None, 
    };
    let owner_res: OwnerOfResponse = router.wrap().query_wasm_smart(&origin_nft_addr, &query_owner_msg).unwrap();
    // Assert that the owner of the token1 is origin_nft_owner
    assert_eq!(origin_nft_owner.to_string(), owner_res.owner);

    let query_owner_msg: cw721_base::msg::QueryMsg<String> = cw721_base::msg::QueryMsg::OwnerOf { 
        token_id: "wrapped_token1".to_string(),
        include_expired: None, 
    };
    let res: Result<OwnerOfResponse, StdError> = router.wrap().query_wasm_smart(&wrapped_nft_addr, &query_owner_msg);
    res.unwrap_err();
}
