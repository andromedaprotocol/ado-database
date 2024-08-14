use cosmwasm_std::{
    Attribute,
    testing::{mock_env, mock_info},
    from_json, Response, DepsMut, MessageInfo, Env,
};
use cw721::Cw721ReceiveMsg;
use andromeda_std::{
    common::encode_binary,
    amp::AndrAddr,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
    error::ContractError,
};
use crate::{
    contract::{instantiate, execute, query},
    msg::{InstantiateMsg, ExecuteMsg, QueryMsg, Cw721HookMsg, WrappedCw721InfoResponse},
    testing::mock_querier::{
        mock_dependencies_custom, MOCK_CW721_CONTRACT, MOCK_WRAPPED_NFT_CONTRACT, 
        MOCK_WRAPPED_TOKEN_OWNER, MOCK_AUTHORIZED_TOKEN1, MOCK_AUTHORIZED_TOKEN2,
    },
}; 

fn init(deps: DepsMut, info: MessageInfo, env: Env, authorized_token_addresses: Option<Vec<AndrAddr>>) -> Response {
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses,
    };

    instantiate(deps, env.clone(), info.clone(), msg).unwrap()
}

fn exec_wrap(deps: DepsMut, info: MessageInfo, env: Env, sender: AndrAddr, token_id: String, unwrappable: bool) -> Result<Response, ContractError> {
    let hook_msg = Cw721HookMsg::MintWrappedNft {
        sender,
        wrapped_token_owner: "wrapped_owner".to_string(),
        unwrappable,
    };

    let msg: ExecuteMsg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_CW721_CONTRACT.to_string(),
        token_id,
        msg: encode_binary(&hook_msg).unwrap(),
    });

    execute(deps, env.clone(), info.clone(), msg)
}

fn exec_unwrap(deps: DepsMut, info: MessageInfo, env: Env) -> Result<Response, ContractError> {
    let unwrap_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_WRAPPED_TOKEN_OWNER.to_string(),
        token_id: "wrapped_token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::UnwrapNft {
            recipient: AndrAddr::from_string("recipient".to_string()),
            wrapped_token: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()),
            wrapped_token_id: "wrapped_token1".to_string(),
        }).unwrap(),
    });

    execute(deps, env.clone(), info.clone(), unwrap_msg)
}

#[test]
fn test_instantiate_with_none_authorized_token_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("owner", &[]);
    let env = mock_env();
    let res = init(
        deps.as_mut(), 
        info.clone(), 
        env.clone(), 
        None
    );
    assert_eq!(0, res.messages.len());

    execute(
        deps.as_mut(), 
        env.clone(), 
        info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let wrapped_token_count_query_msg = QueryMsg::GetWrappedNftCount {};
    let wrapped_token_count_res: u64 = from_json(
        &query(deps.as_ref(), env.clone(), wrapped_token_count_query_msg).unwrap()
    ).unwrap();
    assert_eq!(0, wrapped_token_count_res);

    let wrapped_nft_address_query_msg = QueryMsg::GetWrappedNftAddress {};
    let wrapped_nft_address: AndrAddr = from_json(
        &query(deps.as_ref(), env.clone(), wrapped_nft_address_query_msg).unwrap()
    ).unwrap();
    assert_eq!(MOCK_WRAPPED_NFT_CONTRACT.to_string(), wrapped_nft_address);

    let authorized_token_addresses_query_msg = QueryMsg::GetAuthorizedTokenAddresses {};
    let authorized_token_addresses: Option<Vec<AndrAddr>> = from_json(
        &query(deps.as_ref(), env.clone(), authorized_token_addresses_query_msg).unwrap()
    ).unwrap();
    assert_eq!(None, authorized_token_addresses);
}

#[test]
fn test_instantiate_with_empty_authorized_token_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("owner", &[]);
    let env = mock_env();
    let res = init(
        deps.as_mut(), 
        info.clone(), 
        env.clone(), 
        Some(vec![]),
    );
    assert_eq!(0, res.messages.len());

    let authorized_token_addresses_query_msg = QueryMsg::GetAuthorizedTokenAddresses {};
    let authorized_token_addresses: Option<Vec<AndrAddr>> = from_json(
        &query(deps.as_ref(), env.clone(), authorized_token_addresses_query_msg).unwrap()
    ).unwrap();
    assert_eq!(Some(vec![]), authorized_token_addresses);
}

#[test]
fn test_instantiate_with_authorized_token_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("owner", &[]);
    let env = mock_env();
    let res = init(
        deps.as_mut(), 
        info.clone(), 
        env.clone(),
        Some(vec![
            AndrAddr::from_string(MOCK_AUTHORIZED_TOKEN1.to_string()),
            AndrAddr::from_string(MOCK_AUTHORIZED_TOKEN2.to_string()),
        ]),
    );
    assert_eq!(0, res.messages.len());

    let authorized_token_addresses_query_msg = QueryMsg::GetAuthorizedTokenAddresses {};
    let authorized_token_addresses: Option<Vec<AndrAddr>> = from_json(
        &query(deps.as_ref(), env.clone(), authorized_token_addresses_query_msg).unwrap()
    ).unwrap();
    assert_eq!(
        Some(vec![
            AndrAddr::from_string("mock_authorized_token1".to_string()), 
            AndrAddr::from_string("mock_authorized_token2".to_string()),
        ]), 
        authorized_token_addresses);
}

#[test]
fn test_mint_wrapped_nft_with_none_authorized_token_addresses_at_instance() {
    let mut deps = mock_dependencies_custom(&[]);
    let init_info = mock_info("owner", &[]);
    let env = mock_env();

    init(
        deps.as_mut(), 
        init_info.clone(), 
        env.clone(),
        None,
    );

    execute(
        deps.as_mut(), 
        env.clone(), 
        init_info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let exec_info = mock_info(MOCK_AUTHORIZED_TOKEN1, &[]);
    let res = exec_wrap(
        deps.as_mut(), 
        exec_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_AUTHORIZED_TOKEN1.to_string()),
        "token1".to_string(), 
        true,
    ).unwrap();
    assert_eq!(res.attributes, vec![
        Attribute { key: "method".to_string(), value: "mint_wrapped_cw721".to_string() },
        Attribute { key: "wrapped_token_id".to_string(), value: "wrapped_token1".to_string() }
    ]);

    let exec_info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let res = exec_wrap(
        deps.as_mut(), 
        exec_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        "token2".to_string(),
        true,
    ).unwrap();

    assert_eq!(res.attributes, vec![
        Attribute { key: "method".to_string(), value: "mint_wrapped_cw721".to_string() },
        Attribute { key: "wrapped_token_id".to_string(), value: "wrapped_token2".to_string() }
    ]);

    let wrapped_token_count_res: u64 = from_json(
        &query(deps.as_ref(), env.clone(), QueryMsg::GetWrappedNftCount {}).unwrap()
    ).unwrap();
    assert_eq!(2, wrapped_token_count_res);
}

#[test]
fn test_mint_wrapped_nft_with_authorized_token_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let init_info = mock_info("owner", &[]);
    let env = mock_env();

    init(
        deps.as_mut(), 
        init_info.clone(), 
        env.clone(),
        Some(vec![
            AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        ]),
    );
    execute(
        deps.as_mut(), 
        env.clone(), 
        init_info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let exec_info = mock_info(MOCK_AUTHORIZED_TOKEN1, &[]);
    let err_res = exec_wrap(
        deps.as_mut(), 
        exec_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_AUTHORIZED_TOKEN1.to_string()),
        "token1".to_string(),
        true,
    ).unwrap_err();
    assert_eq!(err_res, ContractError::Unauthorized {});

    let exec_info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let res = exec_wrap(
        deps.as_mut(), 
        exec_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        "token1".to_string(),
        true,
    ).unwrap();

    assert_eq!(res.attributes, vec![
        Attribute { key: "method".to_string(), value: "mint_wrapped_cw721".to_string() },
        Attribute { key: "wrapped_token_id".to_string(), value: "wrapped_token1".to_string() }
    ]);

    let wrapped_token_count_query_msg = QueryMsg::GetWrappedNftCount {};
    let wrapped_token_count_res: u64 = from_json(
        &query(deps.as_ref(), env.clone(), wrapped_token_count_query_msg).unwrap()
    ).unwrap();
    assert_eq!(1, wrapped_token_count_res);

    // Query to verify the wrapped token info
    let query_msg = QueryMsg::GetWrappedCw721 {
        origin_token: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        origin_token_id: "token1".to_string(),
    };

    let res: WrappedCw721InfoResponse = from_json(&query(deps.as_ref(), env.clone(), query_msg).unwrap()).unwrap();
    assert_eq!(res.wrapped_token.to_string(), MOCK_WRAPPED_NFT_CONTRACT.to_string());
    assert_eq!(res.wrapped_token_id, "wrapped_token1".to_string());
}

#[test]
fn test_unwrap_nft() {
    let mut deps = mock_dependencies_custom(&[]);
    let init_info = mock_info("owner", &[]);
    let env = mock_env();

    init(
        deps.as_mut(), 
        init_info.clone(), 
        env.clone(),
        Some(vec![
            AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        ]),
    );
    execute(
        deps.as_mut(), 
        env.clone(), 
        init_info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let wrap_info = mock_info(MOCK_CW721_CONTRACT, &[]);
    exec_wrap(
        deps.as_mut(), 
        wrap_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        "token1".to_string(),
        true,
    ).unwrap();

    let unwrap_info = mock_info(MOCK_WRAPPED_NFT_CONTRACT, &[]);
    let res = exec_unwrap(
        deps.as_mut(), 
        unwrap_info.clone(), 
        env.clone()
    ).unwrap();
    assert_eq!(res.attributes, vec![
        Attribute { key: "method".to_string(), value: "unwrap_cw721".to_string() },
        Attribute { key: "origin_token_id".to_string(), value: "token1".to_string() }
    ]);

    let query_msg = QueryMsg::GetOriginCw721 { 
        wrapped_token: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()),
        wrapped_token_id: "wrapped_token1".to_string(), 
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap_err();
    assert_eq!(res, ContractError::TokenNotWrappedByThisContract {});
}

#[test]
fn test_unwrap_unwrappable_nft() {
    let mut deps = mock_dependencies_custom(&[]);
    let init_info = mock_info("owner", &[]);
    let env = mock_env();

    init(
        deps.as_mut(), 
        init_info.clone(), 
        env.clone(),
        Some(vec![
            AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        ]),
    );
    execute(
        deps.as_mut(), 
        env.clone(), 
        init_info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let wrap_info = mock_info(MOCK_CW721_CONTRACT, &[]);
    exec_wrap(
        deps.as_mut(), 
        wrap_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        "token1".to_string(),
        false,
    ).unwrap();

    let unwrap_info = mock_info(MOCK_WRAPPED_NFT_CONTRACT, &[]);

    let res = exec_unwrap(deps.as_mut(), unwrap_info.clone(), env.clone()).unwrap_err();
    assert_eq!(res, ContractError::UnwrappingDisabled {});
}

#[test]
fn test_query_is_unwrappable() {
    let mut deps = mock_dependencies_custom(&[]);
    let init_info = mock_info("owner", &[]);
    let env = mock_env();

    init(
        deps.as_mut(), 
        init_info.clone(), 
        env.clone(),
        Some(vec![
            AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        ]),
    );
    execute(
        deps.as_mut(), 
        env.clone(), 
        init_info.clone(), 
        ExecuteMsg::SetWrappedNftAddress { 
            wrapped_nft_address: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()) 
        }
    ).unwrap();

    let wrap_info = mock_info(MOCK_CW721_CONTRACT, &[]);
    exec_wrap(
        deps.as_mut(), 
        wrap_info.clone(), 
        env.clone(), 
        AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        "token1".to_string(),
        true,
    ).unwrap();

    let query_msg = QueryMsg::IsUnwrappable {
        wrapped_token: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()),
        wrapped_token_id: "wrapped_token1".to_string(),
    };

    let res: bool = from_json(&query(deps.as_ref(), env.clone(), query_msg).unwrap()).unwrap();
    assert_eq!(res, true);

    let query_msg = QueryMsg::IsUnwrappable {
        wrapped_token: AndrAddr::from_string(MOCK_WRAPPED_NFT_CONTRACT.to_string()),
        wrapped_token_id: "wrapped_token2".to_string(),
    };

    let err_res = query(deps.as_ref(), env.clone(), query_msg).unwrap_err();
    assert_eq!(err_res, ContractError::TokenNotWrappedByThisContract {});
}
