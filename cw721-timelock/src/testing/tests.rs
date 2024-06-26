use cosmwasm_std::{
    Addr, QueryRequest, Attribute,
    testing::{mock_env, mock_info},
    WasmQuery, to_json_binary, from_json, Querier,
};
use cw721::{Cw721QueryMsg, OwnerOfResponse, Cw721ReceiveMsg};
use crate::{
    contract::{instantiate, execute, query},
    msg::{InstantiateMsg, ExecuteMsg, QueryMsg, UnlockTimeResponse, NftDetailsResponse, Cw721HookMsg},
    testing::mock_querier::{MOCK_CW721_CONTRACT, MOCK_TOKEN_OWNER, mock_dependencies_custom}
};
use andromeda_std::{
    common::encode_binary, 
    testing::mock_querier::MOCK_KERNEL_CONTRACT, 
    error::ContractError, 
    common::{milliseconds::MillisecondsDuration},
    amp::AndrAddr,
};

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_timelock_cw721() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Simulate the owner approving the timelock contract as an operator
    // let approval_msg = Cw721ExecuteMsg::Approve {
    //     spender: env.contract.address.to_string(),
    //     token_id: "token1".to_string(),
    //     expires: None,
    // };

    // let approval_info = mock_info(MOCK_TOKEN_OWNER, &[]);
    // let approval_res = execute(deps.as_mut(), env.clone(), approval_info, ExecuteMsg::Cw721 {
    //     contract_addr: MOCK_CW721_CONTRACT.to_string(),
    //     msg: encode_binary(&approval_msg).unwrap(),
    // }).unwrap();

    // println!("Approval response: {:?}", approval_res);

    // println!("Contract Address: {:?}", env.contract.address);


    // let nft_contract = AndrCW721Contract::default();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_CW721_CONTRACT.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(3 * 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap();
    assert_eq!(execute_res.attributes, vec![Attribute { key: "method".to_string(), value: "timelock_cw721".to_string() }, Attribute { key: "lock_id".to_string(), value: "cw721_contract:token1".to_string() }]);
    // Verify the timelock has been set
    let query_res: UnlockTimeResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::UnlockTime {
                cw721_contract: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
                token_id: "token1".to_string(),
            }
        ).unwrap()
    ).unwrap();
    assert_eq!(query_res.unlock_time, env.block.time.seconds() + 3 * 24 * 60 * 60);
}

#[test]
fn test_claim_cw721() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(3 * 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap();
    assert_eq!(execute_res.attributes, vec![
        Attribute { key: "method".to_string(), value: "timelock_cw721".to_string() }, 
        Attribute { key: "lock_id".to_string(), value: "cw721_contract:token1".to_string() }
    ]);

    // Fast forward time
    let mut env_claim = mock_env();
    env_claim.block.time = env.block.time.plus_seconds(300000);

    let claim_msg = ExecuteMsg::ClaimNft {
        cw721_contract: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        token_id: "token1".to_string(),
    };

    let claim_res = execute(deps.as_mut(), env_claim.clone(), info.clone(), claim_msg).unwrap();
    assert_eq!(claim_res.attributes, vec![("method", "claim_nft")]);

    // Verify ownership transfer using raw_query
    let owner_query_msg = to_json_binary(&QueryRequest::<cosmwasm_std::Empty>::Wasm(WasmQuery::Smart {
        contract_addr: MOCK_CW721_CONTRACT.to_string(),
        msg: encode_binary(&Cw721QueryMsg::OwnerOf {
            token_id: "token1".to_string(),
            include_expired: None,
        }).unwrap(),
    })).unwrap();

    let raw_query_res = deps.querier.raw_query(&owner_query_msg);

    let owner_response: OwnerOfResponse = from_json(
        &(raw_query_res.unwrap()).unwrap()
    ).unwrap();

    assert_eq!(owner_response.owner, "owner".to_string());
}

#[test]
fn test_too_short_lock_duration() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(24 * 60 * 60 / 2),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap_err();
    assert_eq!(execute_res, ContractError::LockTimeTooShort {});    
}

#[test]
fn test_too_long_lock_duration() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(2 * 365* 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap_err();
    assert_eq!(execute_res, ContractError::LockTimeTooLong {});    
}

#[test]
fn test_locked_nft() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(3 * 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap();
    assert_eq!(execute_res.attributes, vec![
        Attribute { key: "method".to_string(), value: "timelock_cw721".to_string() }, 
        Attribute { key: "lock_id".to_string(), value: "cw721_contract:token1".to_string() }
    ]);

    // Fast forward time
    let mut env_claim = mock_env();
    env_claim.block.time = env.block.time.plus_seconds(2 * 24 * 60 * 60);

    let claim_msg = ExecuteMsg::ClaimNft {
        cw721_contract: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        token_id: "token1".to_string(),
    };

    let claim_res = execute(deps.as_mut(), env_claim.clone(), info.clone(), claim_msg).unwrap_err();
    assert_eq!(claim_res, ContractError::LockedNFT {});
}

#[test]
fn test_query_nft_details() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(3 * 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap();

    let query_msg = QueryMsg::NftDetails {
        cw721_contract: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        token_id: "token1".to_string(),
    };

    let res: NftDetailsResponse = from_json(
        &query(deps.as_ref(), env.clone(), query_msg).unwrap()
    ).unwrap();

    assert_eq!(res.unlock_time, env.block.time.seconds() + 3 * 24 * 60 * 60);
    assert_eq!(res.recipient, Addr::unchecked("recipient"));
}

#[test]
fn test_query_unlocktime() {
    let mut deps = mock_dependencies_custom(&[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_CW721_CONTRACT.to_string(),
        owner: Some("creator".to_owned()),
        authorized_token_addresses: None,
    };
    let info = mock_info(MOCK_CW721_CONTRACT, &[]);
    let env = mock_env();

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let timelock_cw721_msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        token_id: "token1".to_string(),
        msg: encode_binary(&Cw721HookMsg::TimelockNft {
            lock_duration: MillisecondsDuration::from_seconds(3 * 24 * 60 * 60),
            recipient: AndrAddr::from_string("recipient".to_string()),
        }).unwrap(),
    });

    execute(deps.as_mut(), env.clone(), info.clone(), timelock_cw721_msg).unwrap();
    let query_msg = QueryMsg::UnlockTime {
        cw721_contract: AndrAddr::from_string(MOCK_CW721_CONTRACT.to_string()),
        token_id: "token1".to_string(),
    };

    let res: UnlockTimeResponse = from_json(
        &query(deps.as_ref(), env.clone(), query_msg).unwrap()
    ).unwrap();

    assert_eq!(res.unlock_time, env.block.time.seconds() + 3 * 24 * 60 * 60);
}
