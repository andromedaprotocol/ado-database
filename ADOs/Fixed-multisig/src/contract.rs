use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::state::{next_id, BALLOTS, PROPOSALS, VOTERS};
use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, CONFIG},
};
use andromeda_std::common::encode_binary;
use andromeda_std::error::from_semver;
use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, ado_contract::ADOContract,
    common::context::ExecuteContext,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, Binary, BlockInfo, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order, Response,
    StdResult,
};
use cw2::{get_contract_version, set_contract_version};
use cw3::{
    Ballot, Proposal, ProposalListResponse, ProposalResponse, Status, Vote, VoteInfo,
    VoteListResponse, VoteResponse, VoterDetail, VoterListResponse, VoterResponse, Votes,
};
use cw_storage_plus::Bound;
use cw_utils::{Expiration, ThresholdResponse};

use semver::Version;
use std::cmp::Ordering;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-multisig";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.voters.is_empty() {
        return Err(ContractError::NoVoters {});
    }
    let total_weight = msg.voters.iter().map(|v| v.weight).sum();

    msg.threshold.validate(total_weight)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        threshold: msg.threshold,
        total_weight,
        max_voting_period: msg.max_voting_period,
    };
    CONFIG.save(deps.storage, &cfg)?;
    //add voters
    for voter in msg.voters.iter() {
        let key = deps.api.addr_validate(&voter.addr)?;
        VOTERS.save(deps.storage, &key, &voter.weight)?;
    }

    let contract = ADOContract::default();
    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: "multisig".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    Ok(resp
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);
    match msg {
        ExecuteMsg::AMPReceive(pkt) => Ok(ADOContract::default()
            .execute_amp_receive::<ExecuteMsg, ContractError>(ctx, pkt, handle_execute)?),
        _ => Ok(handle_execute(ctx, msg)?),
    }
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Propose {
            title,
            description,
            msgs,
            latest,
        } => execute_propose(ctx, title, description, msgs, latest),
        ExecuteMsg::Vote { proposal_id, vote } => execute_vote(ctx, proposal_id, vote),
        ExecuteMsg::Execute { proposal_id } => execute_execute(ctx, proposal_id),
        ExecuteMsg::Close { proposal_id } => execute_close(ctx, proposal_id),

        _ => Ok(ADOContract::default().execute(ctx, msg)?),
    }
}

fn execute_propose(
    ctx: ExecuteContext,
    title: String,
    description: String,
    msgs: Vec<CosmosMsg>,
    latest: Option<Expiration>,
) -> Result<Response<Empty>, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let vote_power = VOTERS
        .may_load(deps.storage, &info.sender)?
        .ok_or(ContractError::Unauthorized {})?;

    let cfg = CONFIG.load(deps.storage)?;

    // max expires also used as default
    let max_expires = cfg.max_voting_period.after(&env.block);
    let mut expires = latest.unwrap_or(max_expires);
    let comp = expires.partial_cmp(&max_expires);

    if let Some(Ordering::Greater) = comp {
        expires = max_expires;
    } else if comp.is_none() {
        return Err(ContractError::WrongExpiration {});
    }

    let mut prop = Proposal {
        title,
        description,
        start_height: env.block.height,
        expires,
        msgs,
        status: Status::Open,
        votes: Votes::yes(vote_power),
        threshold: cfg.threshold,
        total_weight: cfg.total_weight,
        proposer: info.sender.clone(),
        deposit: None,
    };
    prop.update_status(&env.block);
    let id = next_id(deps.storage)?;
    PROPOSALS.save(deps.storage, id, &prop)?;

    // add the first yes vote from voter
    let ballot = Ballot {
        weight: vote_power,
        vote: Vote::Yes,
    };
    BALLOTS.save(deps.storage, (id, &info.sender), &ballot)?;

    Ok(Response::new()
        .add_attribute("action", "propose")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", id.to_string())
        .add_attribute("status", format!("{:?}", prop.status)))
}

pub fn execute_vote(
    ctx: ExecuteContext,
    proposal_id: u64,
    vote: Vote,
) -> Result<Response<Empty>, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let voter_power = VOTERS.may_load(deps.storage, &info.sender)?;
    let vote_power = match voter_power {
        Some(power) if power >= 1 => power,
        _ => return Err(ContractError::Unauthorized {}),
    };

    // ensure proposal exists and can be voted on
    let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
    // Allow voting on Passed and Rejected proposals too,
    if ![Status::Open, Status::Passed, Status::Rejected].contains(&prop.status) {
        return Err(ContractError::NotOpen {});
    }
    // if they are not expired
    if prop.expires.is_expired(&env.block) {
        return Err(ContractError::Expired {});
    }

    // cast vote if no vote previously cast
    BALLOTS.update(deps.storage, (proposal_id, &info.sender), |bal| match bal {
        Some(_) => Err(ContractError::AlreadyVoted {}),
        None => Ok(Ballot {
            weight: vote_power,
            vote,
        }),
    })?;

    // update vote tally
    prop.votes.add_vote(vote, vote_power);
    prop.update_status(&env.block);
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    Ok(Response::new()
        .add_attribute("action", "vote")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("status", format!("{:?}", prop.status)))
}

pub fn execute_execute(
    ctx: ExecuteContext,
    proposal_id: u64,
) -> Result<Response<Empty>, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
    // we allow execution even after the proposal "expiration" as long as all vote come in before
    // that point. If it was approved on time, it can be executed any time.
    prop.update_status(&env.block);
    if prop.status != Status::Passed {
        return Err(ContractError::WrongExecuteStatus {});
    }

    // set it to executed
    prop.status = Status::Executed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // dispatch all proposed messages
    Ok(Response::new()
        .add_messages(prop.msgs)
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string()))
}
pub fn execute_close(
    ctx: ExecuteContext,
    proposal_id: u64,
) -> Result<Response<Empty>, ContractError> {
    // anyone can trigger this if the vote passed
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
    if [Status::Executed, Status::Rejected, Status::Passed].contains(&prop.status) {
        return Err(ContractError::WrongCloseStatus {});
    }
    // Avoid closing of Passed due to expiration proposals
    if prop.current_status(&env.block) == Status::Passed {
        return Err(ContractError::WrongCloseStatus {});
    }
    if !prop.expires.is_expired(&env.block) {
        return Err(ContractError::NotExpired {});
    }

    // set it to failed
    prop.status = Status::Rejected;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    Ok(Response::new()
        .add_attribute("action", "close")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    ensure!(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        }
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        }
    );

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Threshold {} => Ok(encode_binary(&query_threshold(deps)?)?),

        QueryMsg::Proposal { proposal_id } => {
            Ok(encode_binary(&query_proposal(deps, env, proposal_id)?)?)
        }

        QueryMsg::Vote { proposal_id, voter } => {
            Ok(encode_binary(&query_vote(deps, proposal_id, voter)?)?)
        }

        QueryMsg::ListProposals { start_after, limit } => Ok(encode_binary(&list_proposals(
            deps,
            env,
            start_after,
            limit,
        )?)?),

        QueryMsg::ReverseProposals {
            start_before,
            limit,
        } => Ok(encode_binary(&reverse_proposals(
            deps,
            env,
            start_before,
            limit,
        )?)?),

        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => Ok(encode_binary(&list_votes(
            deps,
            proposal_id,
            start_after,
            limit,
        )?)?),

        QueryMsg::Voter { address } => Ok(encode_binary(&query_voter(deps, address)?)?),
        QueryMsg::ListVoters { start_after, limit } => {
            Ok(encode_binary(&list_voters(deps, start_after, limit)?)?)
        }
        _ => Ok(ADOContract::default().query(deps, env, msg)?),
    }
}

fn query_threshold(deps: Deps) -> Result<ThresholdResponse, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(cfg.threshold.to_response(cfg.total_weight))
}

fn query_proposal(deps: Deps, env: Env, id: u64) -> Result<ProposalResponse, ContractError> {
    let prop = PROPOSALS.load(deps.storage, id)?;
    let status = prop.current_status(&env.block);
    let threshold = prop.threshold.to_response(prop.total_weight);
    Ok(ProposalResponse {
        id,
        title: prop.title,
        description: prop.description,
        msgs: prop.msgs,
        status,
        expires: prop.expires,
        deposit: prop.deposit,
        proposer: prop.proposer,
        threshold,
    })
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn list_proposals(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<ProposalListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);
    let proposals = PROPOSALS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|p| map_proposal(&env.block, p))
        .collect::<StdResult<_>>()?;

    Ok(ProposalListResponse { proposals })
}

fn reverse_proposals(
    deps: Deps,
    env: Env,
    start_before: Option<u64>,
    limit: Option<u32>,
) -> Result<ProposalListResponse, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let end = start_before.map(Bound::exclusive);
    let props: StdResult<Vec<_>> = PROPOSALS
        .range(deps.storage, None, end, Order::Descending)
        .take(limit)
        .map(|p| map_proposal(&env.block, p))
        .collect();

    Ok(ProposalListResponse { proposals: props? })
}

fn map_proposal(
    block: &BlockInfo,
    item: StdResult<(u64, Proposal)>,
) -> StdResult<ProposalResponse> {
    item.map(|(id, prop)| {
        let status = prop.current_status(block);
        let threshold = prop.threshold.to_response(prop.total_weight);
        ProposalResponse {
            id,
            title: prop.title,
            description: prop.description,
            msgs: prop.msgs,
            status,
            deposit: prop.deposit,
            proposer: prop.proposer,
            expires: prop.expires,
            threshold,
        }
    })
}

fn query_vote(deps: Deps, proposal_id: u64, voter: String) -> Result<VoteResponse, ContractError> {
    let voter = deps.api.addr_validate(&voter)?;
    let ballot = BALLOTS.may_load(deps.storage, (proposal_id, &voter))?;
    let vote = ballot.map(|b| VoteInfo {
        proposal_id,
        voter: voter.into(),
        vote: b.vote,
        weight: b.weight,
    });
    Ok(VoteResponse { vote })
}

fn list_votes(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<VoteListResponse, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::ExclusiveRaw(s.into()));

    let votes = BALLOTS
        .prefix(proposal_id)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            item.map(|(addr, ballot)| VoteInfo {
                proposal_id,
                voter: addr.into(),
                vote: ballot.vote,
                weight: ballot.weight,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(VoteListResponse { votes })
}

fn query_voter(deps: Deps, voter: String) -> Result<VoterResponse, ContractError> {
    let voter = deps.api.addr_validate(&voter)?;
    let weight = VOTERS.may_load(deps.storage, &voter)?;
    Ok(VoterResponse { weight })
}

fn list_voters(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<VoterListResponse, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::ExclusiveRaw(s.into()));

    let voters = VOTERS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            item.map(|(addr, weight)| VoterDetail {
                addr: addr.into(),
                weight,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(VoterListResponse { voters })
}
#[cfg(test)]
mod tests {
    use andromeda_std::testing::mock_querier::MOCK_KERNEL_CONTRACT;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, from_binary, from_json, BankMsg, Decimal};

    use cw2::{get_contract_version, ContractVersion};
    use cw_utils::{Duration, Threshold};

    use crate::msg::Voter;

    use super::*;

    fn mock_env_height(height_delta: u64) -> Env {
        let mut env = mock_env();
        env.block.height += height_delta;
        env
    }

    fn mock_env_time(time_delta: u64) -> Env {
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(time_delta);
        env
    }

    const OWNER: &str = "admin0001";
    const VOTER1: &str = "voter0001";
    const VOTER2: &str = "voter0002";
    const VOTER3: &str = "voter0003";
    const VOTER4: &str = "voter0004";
    const VOTER5: &str = "voter0005";
    const VOTER6: &str = "voter0006";
    const NOWEIGHT_VOTER: &str = "voterxxxx";
    const SOMEBODY: &str = "somebody";

    fn voter<T: Into<String>>(addr: T, weight: u64) -> Voter {
        Voter {
            addr: addr.into(),
            weight,
        }
    }

    // this will set up the instantiation for other tests
    #[track_caller]
    fn setup_test_case(
        deps: DepsMut,
        info: MessageInfo,
        threshold: Threshold,
        max_voting_period: Duration,
    ) -> Result<Response<Empty>, ContractError> {
        // Instantiate a contract with voters
        let voters = vec![
            voter(&info.sender, 1),
            voter(VOTER1, 1),
            voter(VOTER2, 2),
            voter(VOTER3, 3),
            voter(VOTER4, 4),
            voter(VOTER5, 5),
            voter(VOTER6, 1),
            voter(NOWEIGHT_VOTER, 0),
        ];

        let instantiate_msg = InstantiateMsg {
            voters,
            threshold,
            max_voting_period,
            owner: None,
            kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        };
        instantiate(deps, mock_env(), info, instantiate_msg)
    }

    fn get_tally(deps: Deps, proposal_id: u64) -> u64 {
        // Get all the voters on the proposal
        let voters = QueryMsg::ListVotes {
            proposal_id,
            start_after: None,
            limit: None,
        };
        let votes: VoteListResponse = from_json(&query(deps, mock_env(), voters).unwrap()).unwrap();
        // Sum the weights of the Yes votes to get the tally
        votes
            .votes
            .iter()
            .filter(|&v| v.vote == Vote::Yes)
            .map(|v| v.weight)
            .sum()
    }

    #[test]
    fn test_instantiate_works() {
        let mut deps = mock_dependencies();
        let info = mock_info(OWNER, &[]);

        let max_voting_period = Duration::Time(1234567);

        // No voters fails
        let instantiate_msg = InstantiateMsg {
            voters: vec![],
            threshold: Threshold::ThresholdQuorum {
                threshold: Decimal::zero(),
                quorum: Decimal::percent(1),
            },
            max_voting_period,
            owner: None,
            kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        };
        let err = instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            instantiate_msg.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::NoVoters {});

        // Zero required weight fails
        let instantiate_msg = InstantiateMsg {
            voters: vec![voter(OWNER, 1)],
            ..instantiate_msg
        };
        let err =
            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::Threshold(cw_utils::ThresholdError::InvalidThreshold {})
        );

        // Total weight less than required weight not allowed
        let threshold = Threshold::AbsoluteCount { weight: 100 };
        let err =
            setup_test_case(deps.as_mut(), info.clone(), threshold, max_voting_period).unwrap_err();
        assert_eq!(
            err,
            ContractError::Threshold(cw_utils::ThresholdError::UnreachableWeight {})
        );

        // All valid
        let threshold = Threshold::AbsoluteCount { weight: 1 };
        setup_test_case(deps.as_mut(), info, threshold, max_voting_period).unwrap();

        // Verify
        assert_eq!(
            ContractVersion {
                contract: CONTRACT_NAME.to_string(),
                version: CONTRACT_VERSION.to_string(),
            },
            get_contract_version(&deps.storage).unwrap()
        )
    }

    // TODO: query() tests

    #[test]
    fn zero_weight_member_cant_vote() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::AbsoluteCount { weight: 4 };
        let voting_period = Duration::Time(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info, threshold, voting_period).unwrap();

        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];

        // Voter without voting power still can create proposal
        let info = mock_info(NOWEIGHT_VOTER, &[]);
        let proposal = ExecuteMsg::Propose {
            title: "Rewarding somebody".to_string(),
            description: "Do we reward her?".to_string(),
            msgs,
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Cast a No vote
        let no_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::No,
        };
        // Only voters with weight can vote
        let info = mock_info(NOWEIGHT_VOTER, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, no_vote).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn test_propose_works() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::AbsoluteCount { weight: 4 };
        let voting_period = Duration::Time(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info, threshold, voting_period).unwrap();

        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];

        // Only voters can propose
        let info = mock_info(SOMEBODY, &[]);
        let proposal = ExecuteMsg::Propose {
            title: "Rewarding somebody".to_string(),
            description: "Do we reward her?".to_string(),
            msgs: msgs.clone(),
            latest: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, proposal.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // Wrong expiration option fails
        let info = mock_info(OWNER, &[]);
        let proposal_wrong_exp = ExecuteMsg::Propose {
            title: "Rewarding somebody".to_string(),
            description: "Do we reward her?".to_string(),
            msgs,
            latest: Some(Expiration::AtHeight(123456)),
        };
        let err = execute(deps.as_mut(), mock_env(), info, proposal_wrong_exp).unwrap_err();
        assert_eq!(err, ContractError::WrongExpiration {});

        // Proposal from voter works
        let info = mock_info(VOTER3, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, proposal.clone()).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "propose")
                .add_attribute("sender", VOTER3)
                .add_attribute("proposal_id", 1.to_string())
                .add_attribute("status", "Open")
        );

        // Proposal from voter with enough vote power directly passes
        let info = mock_info(VOTER4, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, proposal).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "propose")
                .add_attribute("sender", VOTER4)
                .add_attribute("proposal_id", 2.to_string())
                .add_attribute("status", "Passed")
        );
    }

    #[test]
    fn test_vote_works() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::AbsoluteCount { weight: 3 };
        let voting_period = Duration::Time(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info.clone(), threshold, voting_period).unwrap();

        // Propose
        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];
        let proposal = ExecuteMsg::Propose {
            title: "Pay somebody".to_string(),
            description: "Do I pay her?".to_string(),
            msgs,
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Owner cannot vote (again)
        let yes_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::Yes,
        };
        let err = execute(deps.as_mut(), mock_env(), info, yes_vote.clone()).unwrap_err();
        assert_eq!(err, ContractError::AlreadyVoted {});

        // Only voters can vote
        let info = mock_info(SOMEBODY, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, yes_vote.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // But voter1 can
        let info = mock_info(VOTER1, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, yes_vote.clone()).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER1)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Open")
        );

        // No/Veto votes have no effect on the tally
        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Compute the current tally
        let tally = get_tally(deps.as_ref(), proposal_id);

        // Cast a No vote
        let no_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::No,
        };
        let info = mock_info(VOTER2, &[]);
        execute(deps.as_mut(), mock_env(), info, no_vote.clone()).unwrap();

        // Cast a Veto vote
        let veto_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::Veto,
        };
        let info = mock_info(VOTER3, &[]);
        execute(deps.as_mut(), mock_env(), info.clone(), veto_vote).unwrap();

        // Verify
        assert_eq!(tally, get_tally(deps.as_ref(), proposal_id));

        // Once voted, votes cannot be changed
        let err = execute(deps.as_mut(), mock_env(), info.clone(), yes_vote.clone()).unwrap_err();
        assert_eq!(err, ContractError::AlreadyVoted {});
        assert_eq!(tally, get_tally(deps.as_ref(), proposal_id));

        // Expired proposals cannot be voted
        let env = match voting_period {
            Duration::Time(duration) => mock_env_time(duration + 1),
            Duration::Height(duration) => mock_env_height(duration + 1),
        };
        let err = execute(deps.as_mut(), env, info, no_vote).unwrap_err();
        assert_eq!(err, ContractError::Expired {});

        // Vote it again, so it passes
        let info = mock_info(VOTER4, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, yes_vote.clone()).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER4)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Passed")
        );

        // Passed proposals can still be voted (while they are not expired or executed)
        let info = mock_info(VOTER5, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, yes_vote).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER5)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Passed")
        );

        // Propose
        let info = mock_info(OWNER, &[]);
        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];
        let proposal = ExecuteMsg::Propose {
            title: "Pay somebody".to_string(),
            description: "Do I pay her?".to_string(),
            msgs,
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Cast a No vote
        let no_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::No,
        };
        // Voter1 vote no, weight 1
        let info = mock_info(VOTER1, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, no_vote.clone()).unwrap();

        // Verify it is not enough to reject yet
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER1)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Open")
        );

        // Voter 4 votes no, weight 4, total weight for no so far 5, need 14 to reject
        let info = mock_info(VOTER4, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, no_vote.clone()).unwrap();

        // Verify it is still open as we actually need no votes > 17 - 3
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER4)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Open")
        );

        // Voter 3 votes no, weight 3, total weight for no far 8, need 14
        let info = mock_info(VOTER3, &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, no_vote.clone()).unwrap();

        // Voter 5 votes no, weight 5, total weight for no far 13, need 14
        let info = mock_info(VOTER5, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, no_vote.clone()).unwrap();

        // Verify it is still open as we actually need no votes > 17 - 3
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER5)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Open")
        );

        // Voter 2 votes no, weight 2, total weight for no so far 15, need 14.
        // Can now reject
        let info = mock_info(VOTER2, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, no_vote).unwrap();

        // Verify it is rejected as, 15 no votes > 17 - 3
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER2)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Rejected")
        );

        // Rejected proposals can still be voted (while they are not expired)
        let info = mock_info(VOTER6, &[]);
        let yes_vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::Yes,
        };
        let res = execute(deps.as_mut(), mock_env(), info, yes_vote).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER6)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Rejected")
        );
    }

    #[test]
    fn test_execute_works() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::AbsoluteCount { weight: 3 };
        let voting_period = Duration::Time(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info.clone(), threshold, voting_period).unwrap();

        // Propose
        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];
        let proposal = ExecuteMsg::Propose {
            title: "Pay somebody".to_string(),
            description: "Do I pay her?".to_string(),
            msgs: msgs.clone(),
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Only Passed can be executed
        let execution = ExecuteMsg::Execute { proposal_id };
        let err = execute(deps.as_mut(), mock_env(), info, execution.clone()).unwrap_err();
        assert_eq!(err, ContractError::WrongExecuteStatus {});

        // Vote it, so it passes
        let vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::Yes,
        };
        let info = mock_info(VOTER3, &[]);
        let res = execute(deps.as_mut(), mock_env(), info.clone(), vote).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER3)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Passed")
        );

        // In passing: Try to close Passed fails
        let closing = ExecuteMsg::Close { proposal_id };
        let err = execute(deps.as_mut(), mock_env(), info, closing).unwrap_err();
        assert_eq!(err, ContractError::WrongCloseStatus {});

        // Execute works. Anybody can execute Passed proposals
        let info = mock_info(SOMEBODY, &[]);
        let res = execute(deps.as_mut(), mock_env(), info.clone(), execution).unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_messages(msgs)
                .add_attribute("action", "execute")
                .add_attribute("sender", SOMEBODY)
                .add_attribute("proposal_id", proposal_id.to_string())
        );

        // In passing: Try to close Executed fails
        let closing = ExecuteMsg::Close { proposal_id };
        let err = execute(deps.as_mut(), mock_env(), info, closing).unwrap_err();
        assert_eq!(err, ContractError::WrongCloseStatus {});
    }

    #[test]
    fn proposal_pass_on_expiration() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::ThresholdQuorum {
            threshold: Decimal::percent(51),
            quorum: Decimal::percent(1),
        };
        let voting_period = Duration::Time(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info.clone(), threshold, voting_period).unwrap();

        // Propose
        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];
        let proposal = ExecuteMsg::Propose {
            title: "Pay somebody".to_string(),
            description: "Do I pay her?".to_string(),
            msgs,
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        // Vote it, so it passes after voting period is over
        let vote = ExecuteMsg::Vote {
            proposal_id,
            vote: Vote::Yes,
        };
        let info = mock_info(VOTER3, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, vote).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "vote")
                .add_attribute("sender", VOTER3)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("status", "Open")
        );

        // Wait until the voting period is over
        let env = match voting_period {
            Duration::Time(duration) => mock_env_time(duration + 1),
            Duration::Height(duration) => mock_env_height(duration + 1),
        };

        // Proposal should now be passed
        let prop: ProposalResponse = from_json(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::Proposal { proposal_id },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(prop.status, Status::Passed);

        // Closing should NOT be possible
        let info = mock_info(SOMEBODY, &[]);
        let err = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::Close { proposal_id },
        )
        .unwrap_err();
        assert_eq!(err, ContractError::WrongCloseStatus {});

        // Execution should now be possible
        let res = execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::Execute { proposal_id },
        )
        .unwrap();
        assert_eq!(
            res.attributes,
            Response::<Empty>::new()
                .add_attribute("action", "execute")
                .add_attribute("sender", SOMEBODY)
                .add_attribute("proposal_id", proposal_id.to_string())
                .attributes
        )
    }

    #[test]
    fn test_close_works() {
        let mut deps = mock_dependencies();

        let threshold = Threshold::AbsoluteCount { weight: 3 };
        let voting_period = Duration::Height(2000000);

        let info = mock_info(OWNER, &[]);
        setup_test_case(deps.as_mut(), info.clone(), threshold, voting_period).unwrap();

        // Propose
        let bank_msg = BankMsg::Send {
            to_address: SOMEBODY.into(),
            amount: vec![coin(1, "BTC")],
        };
        let msgs = vec![CosmosMsg::Bank(bank_msg)];
        let proposal = ExecuteMsg::Propose {
            title: "Pay somebody".to_string(),
            description: "Do I pay her?".to_string(),
            msgs: msgs.clone(),
            latest: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        let closing = ExecuteMsg::Close { proposal_id };

        // Anybody can close
        let info = mock_info(SOMEBODY, &[]);

        // Non-expired proposals cannot be closed
        let err = execute(deps.as_mut(), mock_env(), info, closing).unwrap_err();
        assert_eq!(err, ContractError::NotExpired {});

        // Expired proposals can be closed
        let info = mock_info(OWNER, &[]);

        let proposal = ExecuteMsg::Propose {
            title: "(Try to) pay somebody".to_string(),
            description: "Pay somebody after time?".to_string(),
            msgs,
            latest: Some(Expiration::AtHeight(123456)),
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), proposal).unwrap();

        // Get the proposal id from the logs
        let proposal_id: u64 = res.attributes[2].value.parse().unwrap();

        let closing = ExecuteMsg::Close { proposal_id };

        // Close expired works
        let env = mock_env_height(1234567);
        let res = execute(
            deps.as_mut(),
            env,
            mock_info(SOMEBODY, &[]),
            closing.clone(),
        )
        .unwrap();

        // Verify
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "close")
                .add_attribute("sender", SOMEBODY)
                .add_attribute("proposal_id", proposal_id.to_string())
        );

        // Trying to close it again fails
        let err = execute(deps.as_mut(), mock_env(), info, closing).unwrap_err();
        assert_eq!(err, ContractError::WrongCloseStatus {});
    }
}
