use cosmwasm_std::{
    from_json, to_json_binary,
    testing::{mock_env, mock_info, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
    Binary, Coin, ContractResult, OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery, QuerierWrapper,
};
use cw721::{Cw721QueryMsg, OwnerOfResponse, NftInfoResponse};
use andromeda_std::{
    ado_contract::ADOContract,
    ado_base::InstantiateMsg,
    os::kernel::{QueryMsg as KernelQueryMsg, ChannelInfo},
    amp::{VFS_KEY, ADO_DB_KEY, ECONOMICS_KEY, OSMOSIS_ROUTER_KEY},
    testing::mock_querier::{MOCK_APP_CONTRACT, MOCK_KERNEL_CONTRACT},
};
use andromeda_app::app::QueryMsg as AppQueryMsg;
use andromeda_non_fungible_tokens::cw721::TokenExtension;

pub const MOCK_CW721_CONTRACT: &str = "cw721_contract";
pub const MOCK_WRAPPED_NFT_CONTRACT: &str = "wrapped_nft_contract";
pub const MOCK_TOKEN_OWNER: &str = "owner";
pub const MOCK_WRAPPED_TOKEN_OWNER: &str = "wrapped_owner";
pub const MOCK_UNCLAIMED_TOKEN: &str = "unclaimed_token";
pub const MOCK_AUTHORIZED_TOKEN1: &str = "mock_authorized_token1";
pub const MOCK_AUTHORIZED_TOKEN2: &str = "mock_authorized_token2";

/// An invalid contract address
pub const INVALID_CONTRACT: &str = "invalid_contract";
/// Mock VFS Contract Address
pub const MOCK_VFS_CONTRACT: &str = "vfs_contract";
/// Mock ADODB Contract Address
pub const MOCK_ADODB_CONTRACT: &str = "adodb_contract";
// Mock Osmosis Router
pub const MOCK_OSMOSIS_ROUTER_CONTRACT: &str = "osmosis_router";
// Mock Economics Contract
pub const MOCK_ECONOMICS_CONTRACT: &str = "economics_contract";

// Mock dependencies with custom querier
pub fn mock_dependencies_custom(contract_balance: &[Coin]) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));
    let storage = MockStorage::default();
    let mut deps = OwnedDeps {
        storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: std::marker::PhantomData,
    };
    ADOContract::default()
        .instantiate(
            &mut deps.storage,
            mock_env(),
            &deps.api,
            &QuerierWrapper::new(&deps.querier),
            mock_info("sender", &[]),
            InstantiateMsg {
                ado_type: "wrapped-cw721".to_string(),
                ado_version: "test".to_string(),
                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();
    deps
}

pub struct WasmMockQuerier {
    pub base: MockQuerier,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<cosmwasm_std::Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {e}"),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<cosmwasm_std::Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match contract_addr.as_str() {
                    MOCK_CW721_CONTRACT => self.handle_token_query(msg),
                    MOCK_AUTHORIZED_TOKEN1 => self.handle_token_query(msg),
                    MOCK_AUTHORIZED_TOKEN2 => self.handle_token_query(msg),
                    MOCK_APP_CONTRACT => self.handle_app_query(msg),
                    MOCK_KERNEL_CONTRACT => self.handle_kernel_query(msg),
                    _ => SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: format!("{:?}", request),
                    }),
                }
            }
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                match contract_addr.as_str() {
                    MOCK_KERNEL_CONTRACT => self.handle_kernel_raw_query(key, false),
                    _ => SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: format!("{:?}", request),
                    }),
                }
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: format!("{:?}", request),
            }),
        }
    }

    fn handle_app_query(&self, msg: &Binary) -> QuerierResult {
        let valid_identifiers = ["e", "b"];
        match from_json(msg).unwrap() {
            AppQueryMsg::ComponentExists { name } => {
                let value = valid_identifiers.contains(&name.as_str());
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&value).unwrap()))
            }
            _ => panic!("Unsupported Query: {msg}"),
        }
    }

    fn handle_token_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            Cw721QueryMsg::OwnerOf { token_id, .. } => {
                let res = if token_id == MOCK_UNCLAIMED_TOKEN {
                    OwnerOfResponse {
                        owner: mock_env().contract.address.to_string(),
                        approvals: vec![],
                    }
                } else {
                    OwnerOfResponse {
                        owner: MOCK_TOKEN_OWNER.to_owned(),
                        approvals: vec![],
                    }
                };
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
            },
            Cw721QueryMsg::NftInfo { .. } => {
                let res = NftInfoResponse{
                    token_uri: None,
                    extension: TokenExtension {
                        publisher: "Andromeda".to_string(),
                    } 
                };
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
            },

            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: format!("{:?}", msg),
            }),
        }
    }

    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier {
            base,
        }
    }

    fn handle_kernel_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            KernelQueryMsg::KeyAddress { key } => match key.as_str() {
                VFS_KEY => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&MOCK_VFS_CONTRACT).unwrap(),
                )),
                ADO_DB_KEY => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&MOCK_ADODB_CONTRACT).unwrap(),
                )),
                &_ => SystemResult::Ok(ContractResult::Err("Invalid Key".to_string())),
            },
            KernelQueryMsg::VerifyAddress { address } => match address.as_str() {
                INVALID_CONTRACT => {
                    SystemResult::Ok(ContractResult::Err("Invalid Address".to_string()))
                }
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&true).unwrap())),
            },
            _ => SystemResult::Ok(ContractResult::Err("Not implemented".to_string())),
        }
    }

    pub fn handle_kernel_raw_query(&self, key: &Binary, fake: bool) -> QuerierResult {
        let key_vec = key.as_slice();
        let key_str = String::from_utf8(key_vec.to_vec()).unwrap();

        if key_str.contains("kernel_addresses") {
            let split = key_str.split("kernel_addresses");
            let key = split.last();
            if let Some(key) = key {
                match key {
                    VFS_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_VFS_CONTRACT.to_string()).unwrap(),
                    )),
                    ADO_DB_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_ADODB_CONTRACT.to_string()).unwrap(),
                    )),
                    OSMOSIS_ROUTER_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_OSMOSIS_ROUTER_CONTRACT.to_string()).unwrap(),
                    )),
                    ECONOMICS_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_ECONOMICS_CONTRACT.to_string()).unwrap(),
                    )),
                    _ => panic!("Invalid Kernel Address Key"),
                }
            } else {
                panic!("Invalid Kernel Address Raw Query")
            }
        } else if key_str.contains("curr_chain") {
            let res = if fake {
                "fake_chain".to_string()
            } else {
                "andromeda".to_string()
            };
            SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
        } else if key_str.contains("channel") {
            SystemResult::Ok(ContractResult::Ok(
                to_json_binary(&ChannelInfo {
                    kernel_address: "kernel".to_string(),
                    ics20_channel_id: Some("1".to_string()),
                    direct_channel_id: Some("2".to_string()),
                    supported_modules: vec![],
                })
                .unwrap(),
            ))
        } else {
            panic!("Invalid Kernel Raw Query")
        }
    }
}