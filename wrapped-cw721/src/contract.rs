#[cfg(not(feature = "imported"))]
use cosmwasm_std::{
    entry_point, ensure, from_json, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, WasmMsg,
};
use andromeda_non_fungible_tokens::cw721::{
    ExecuteMsg as AndrCw721ExecuteMsg, TokenExtension,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg, permissioning::Permission},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{encode_binary, context::ExecuteContext},
    error::ContractError,
};
use cw721::Cw721ReceiveMsg;

use crate::{
    msg::{Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, OriginCw721InfoResponse, WrappedCw721InfoResponse},
    state::{WrappedInfo, WRAPPED_INFO, OriginInfo, ORIGIN_INFO, WRAPPED_NFT_ADDRESS, WRAPPED_NFT_COUNT, AUTHORIED_TOKEN_ADDRESSES},
};

const CONTRACT_NAME: &str = "crates.io:andromeda-wrapped-cw721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_WRAPPED_TOKEN_COUNT: u64 = 0;
const SEND_NFT_ACTION: &str = "SEND_NFT";

#[cfg_attr(not(feature = "imported"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    let contract = ADOContract::default();

    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    // Set send action permission to origin nfts.
    if let Some(authorized_token_addresses) = msg.authorized_token_addresses {

        AUTHORIED_TOKEN_ADDRESSES.save(deps.storage, &authorized_token_addresses)?;

        if !authorized_token_addresses.is_empty() {
            ADOContract::default().permission_action(SEND_NFT_ACTION, deps.storage)?;
        }

        for token_address in authorized_token_addresses {
            let addr = token_address.get_raw_address(&deps.as_ref())?;
            ADOContract::set_permission(
                deps.storage,
                SEND_NFT_ACTION,
                addr,
                Permission::Whitelisted(None),
            )?;
        }
    }
    WRAPPED_NFT_COUNT.save(deps.storage, &DEFAULT_WRAPPED_TOKEN_COUNT)?;

    Ok(
        resp
        .add_attribute("method", "instantiate")
    )
}

#[cfg_attr(not(feature = "imported"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);
    if let ExecuteMsg::AMPReceive(pkt) = msg {
        ADOContract::default().execute_amp_receive(
            ctx,
            pkt,
            handle_execute,
        )
    } else {
        handle_execute(ctx, msg)
    }
}

fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {

    match msg {
        ExecuteMsg::ReceiveNft(msg) => handle_receive_cw721(ctx, msg),
        ExecuteMsg::SetWrappedNftAddress { wrapped_nft_address } => execute_set_wrapped_nft_address(ctx, wrapped_nft_address),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn execute_set_wrapped_nft_address(
    ctx: ExecuteContext,
    wrapped_nft_address: AndrAddr,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps,
        ..
    } = ctx;

    WRAPPED_NFT_ADDRESS.save(deps.storage, &wrapped_nft_address)?;

    let addr = wrapped_nft_address.get_raw_address(&deps.as_ref())?;
    ADOContract::set_permission(
        deps.storage, 
        SEND_NFT_ACTION, 
        addr,
        Permission::Whitelisted(None),
    )?;

    Ok(Response::new()
        .add_attribute("method", "set_wrapped_nft_address")
        .add_attribute("wrapped_nft_address", wrapped_nft_address.to_string())
    )
}

fn handle_receive_cw721(ctx: ExecuteContext, msg: Cw721ReceiveMsg) -> Result<Response, ContractError> {

    match from_json(&msg.msg)? {
        Cw721HookMsg::MintWrappedNft { 
            sender,
            wrapped_token_owner,
            unwrappable,
        } => execute_mint_wrapped_cw721(
            ctx,
            sender,
            msg.token_id,
            wrapped_token_owner,
            unwrappable,
        ),
        Cw721HookMsg::UnwrapNft { 
            recipient,
            wrapped_token,
            wrapped_token_id 
        } => execute_unwrap_cw721(
            ctx,
            recipient,
            wrapped_token,
            wrapped_token_id,
        ),
    }
}

fn execute_mint_wrapped_cw721(
    ctx: ExecuteContext,
    origin_token: AndrAddr,
    origin_token_id: String,
    wrapped_token_owner: String,
    unwrappable: bool,
) -> Result<Response, ContractError> {

    let ExecuteContext {
        deps,
        info,
        ..
    } = ctx;

    ADOContract::default().is_permissioned(
        deps.storage,
        ctx.env.clone(),
        SEND_NFT_ACTION,
        info.sender.clone(),
    )?;

    ensure!(
        info.sender.to_string() == origin_token.to_string(),
        ContractError::VerificationFailed {}
    );

    let wrapped_token = WRAPPED_NFT_ADDRESS.load(deps.storage)?;

    let current_wrapped_token_count = WRAPPED_NFT_COUNT.load(deps.storage)?;
    let wrapped_token_count = current_wrapped_token_count + 1;
    let wrapped_token_id = format!("{}{}", "wrapped_token", wrapped_token_count.to_string());

    let wrapped_id = (&wrapped_token.get_raw_address(&deps.as_ref())?, wrapped_token_id.as_str());
    let origin_id = (&origin_token.get_raw_address(&deps.as_ref())?, origin_token_id.as_str());

    let origin_info = OriginInfo {
        origin_token: origin_token.clone(),
        origin_token_id: origin_token_id.clone(),
        unwrappable: unwrappable.clone(),
    };
    let wrapped_info = WrappedInfo {
        wrapped_token: wrapped_token.clone(),
        wrapped_token_id: wrapped_token_id.clone(),
        unwrappable: unwrappable.clone(),
    };

    ORIGIN_INFO.save(deps.storage, wrapped_id, &origin_info)?;
    WRAPPED_INFO.save(deps.storage, origin_id, &wrapped_info)?;

    let wrapped_token_extension = TokenExtension {
        publisher: "Andromeda".to_string(),
    };

    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: wrapped_token.into_string().clone(),
        msg: encode_binary(&AndrCw721ExecuteMsg::Mint { 
            token_id: wrapped_token_id.clone(), 
            owner: wrapped_token_owner.clone(), 
            token_uri: None, 
            extension: wrapped_token_extension,
        })?,
        funds: vec![],
    });

    WRAPPED_NFT_COUNT.save(deps.storage, &wrapped_token_count)?;

    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("method", "mint_wrapped_cw721")
        .add_attribute("wrapped_token_id", wrapped_token_id)
    )
}

fn execute_unwrap_cw721(
    ctx: ExecuteContext,
    recipient: AndrAddr,
    wrapped_token: AndrAddr,
    wrapped_token_id: String,
) -> Result<Response, ContractError> {

    let ExecuteContext {
        deps,
        info,
        ..
    } = ctx;

    ADOContract::default().is_permissioned(
        deps.storage,
        ctx.env.clone(),
        SEND_NFT_ACTION,
        info.sender.clone(),
    )?;

    let wrapped_id = (&wrapped_token.get_raw_address(&deps.as_ref())?, wrapped_token_id.as_str());
    let origin_info = ORIGIN_INFO
    .load(deps.storage, wrapped_id)
    .map_err(|_| ContractError::TokenNotWrappedByThisContract {})?;

    ensure!(
        origin_info.unwrappable,
        ContractError::UnwrappingDisabled {}
    );

    let origin_token = origin_info.origin_token;
    let origin_token_id = origin_info.origin_token_id;
    let origin_id = (&origin_token.get_raw_address(&deps.as_ref())?, origin_token_id.as_str());

    let unwrap_cw721_msg = CosmosMsg::Wasm(WasmMsg::Execute { 
        contract_addr: origin_token.into_string().clone(), 
        msg: encode_binary(&AndrCw721ExecuteMsg::TransferNft { 
            recipient: AndrAddr::from_string(recipient.to_string()), 
            token_id: origin_token_id.clone(), 
        })?, 
        funds: vec![], 
    });

    let burn_wrapped_cw721_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: wrapped_token.into_string().clone(),
        msg: encode_binary(&AndrCw721ExecuteMsg::Burn { token_id: wrapped_token_id.clone() })?,
        funds: vec![],
    });

    WRAPPED_INFO.remove(deps.storage, origin_id);
    ORIGIN_INFO.remove(deps.storage, wrapped_id);

    Ok(
        Response::new()
        .add_message(burn_wrapped_cw721_msg)
        .add_message(unwrap_cw721_msg)
        .add_attribute("method", "unwrap_cw721")
        .add_attribute("origin_token_id", origin_token_id)
    )

}

#[cfg_attr(not(feature = "imported"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg{
        QueryMsg::GetOriginCw721 { wrapped_token, wrapped_token_id } => {
            Ok(encode_binary(&query_origin_cw721(deps, wrapped_token, wrapped_token_id)?)?)
        },
        QueryMsg::GetWrappedCw721 { origin_token, origin_token_id } => {
            Ok(encode_binary(&query_wrapped_cw721(deps, origin_token, origin_token_id)?)?)
        },
        QueryMsg::IsUnwrappable { wrapped_token, wrapped_token_id } => {
            Ok(encode_binary(&query_is_unwrappable(deps, wrapped_token, wrapped_token_id)?)?)
        },
        QueryMsg::GetWrappedNftCount {} => {
            Ok(encode_binary(&query_wrapped_nft_count(deps)?)?)
        },
        QueryMsg::GetWrappedNftAddress {} => {
            Ok(encode_binary(&query_wrapped_nft_address(deps)?)?)
        },
        QueryMsg::GetAuthorizedTokenAddresses {} => {
            Ok(encode_binary(&query_authorized_token_addresses(deps)?)?)
        },
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn query_origin_cw721(
    deps: Deps, 
    wrapped_token: AndrAddr, 
    wrapped_token_id: String
) -> Result<OriginCw721InfoResponse, ContractError> {
    let wrapped_id = (&wrapped_token.get_raw_address(&deps)?, wrapped_token_id.as_str());
    let origin_info = ORIGIN_INFO
    .load(deps.storage, wrapped_id)
    .map_err(|_| ContractError::TokenNotWrappedByThisContract {})?;

    Ok(OriginCw721InfoResponse {
        origin_token: origin_info.origin_token,
        origin_token_id: origin_info.origin_token_id,
    })
}

pub fn query_wrapped_cw721(
    deps: Deps, 
    origin_token: AndrAddr, 
    origin_token_id: String
) -> Result<WrappedCw721InfoResponse, ContractError> {
    let origin_id = (&origin_token.get_raw_address(&deps)?, origin_token_id.as_str());
    let wrapped_info = WRAPPED_INFO
    .load(deps.storage, origin_id)
    .map_err(|_| ContractError::TokenNotWrappedByThisContract {})?;

    Ok(WrappedCw721InfoResponse {
        wrapped_token: wrapped_info.wrapped_token,
        wrapped_token_id: wrapped_info.wrapped_token_id,
    })
}

pub fn query_is_unwrappable(
    deps: Deps, 
    wrapped_token: AndrAddr, 
    wrapped_token_id: String
) -> Result<bool, ContractError> {
    let wrapped_id = (&wrapped_token.get_raw_address(&deps)?, wrapped_token_id.as_str());
    let origin_info = ORIGIN_INFO
    .load(deps.storage, wrapped_id)
    .map_err(|_| ContractError::TokenNotWrappedByThisContract {})?;

    Ok(origin_info.unwrappable)
}

pub fn query_wrapped_nft_count(deps: Deps) -> Result<u64, ContractError> {
    let wrapped_nft_count = WRAPPED_NFT_COUNT.load(deps.storage)?;
    Ok(wrapped_nft_count)
}

pub fn query_wrapped_nft_address(deps: Deps)  -> Result<AndrAddr, ContractError> {
    let wrapped_nft_address = WRAPPED_NFT_ADDRESS.load(deps.storage)?;
    Ok(wrapped_nft_address)
}

pub fn query_authorized_token_addresses(deps: Deps) -> Result<Option<Vec<AndrAddr>>, ContractError> {
    let authorized_token_addresses = AUTHORIED_TOKEN_ADDRESSES.may_load(deps.storage)?;
    Ok(authorized_token_addresses)
}

#[cfg_attr(not(feature = "imported"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}
