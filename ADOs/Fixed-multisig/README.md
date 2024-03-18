# ADO Purpose
The Fixed Multisig ADO is designed to allow the execution of some action/actions (CosmosMsg) on another smart contract or wallet by the process of voting. The overall purpose of the ADO is to handle  multi-ownership scenarios, whether on a wallet or another smart contract.   
A fixed set of addresses are specified that are allowed to vote. For each Address a specific weight can be assigned. Proposals can be started by any of the voters to execute some action. The owner of the ADO sets a threshold to be met for a proposal to pass. Once passed the action can now be executed by anyone.

### Messages

**Instantiation (What is specified and stored at instantiation)**
```
pub struct InstantiateMsg {
    pub voters: Vec<Voter>,
    pub threshold: Threshold,
    pub max_voting_period: Duration,
    pub kernel_address: String,
    pub owner: Option<String>,
}
```
**voters**: A vector of the struct Voter. Defines set of addresses allowed to vote. Each address has a weight for their vote. A weight of 0 can be set for a voter allowing them to start proposals without having any voting power.
```
#[cw_serde]
pub struct Voter {
    pub addr: String,
    pub weight: u64,
}
```
**addr**: The address of the voter.
**weight**: The weight of the voter

**Theshold**: The threshold needed for a proposal to pass. There are three types of thresholds to choose from.
```
pub enum Threshold {
    /// Declares that a fixed weight of Yes votes is needed to pass.
    /// See `ThresholdResponse.AbsoluteCount` in the cw3 spec for details.
    AbsoluteCount { weight: u64 },

    /// Declares a percentage of the total weight that must cast Yes votes in order for
    /// a proposal to pass.
    /// See `ThresholdResponse.AbsolutePercentage` in the cw3 spec for details.
    AbsolutePercentage { percentage: Decimal },

    /// Declares a `quorum` of the total votes that must participate in the election in order
    /// for the vote to be considered at all.
    /// See `ThresholdResponse.ThresholdQuorum` in the cw3 spec for details.
    ThresholdQuorum { threshold: Decimal, quorum: Decimal },
}
```
**max_voting_period**: Specifies a Duration which indicates maximum duration a proposal can be up for. The max duration can be either a block height, or a time specified in seconds. 
```
pub enum Duration {
    Height(u64),
    /// Time in seconds
    Time(u64),
}
```

**Execute Messages (What are the messages that can be executed, what do they do, and who can call each message)**

1. Propose: Message to start a proposal. Only addresses that were specified as voters are allowed to start a proposal. Multiple proposals can be running at the same time. When a proposal is made, it is assigned an Id which starts 1 and increments for each new proposal. 
```
  Propose {
        title: String,
        description: String,
        msgs: Vec<CosmosMsg<Empty>>,
        latest: Option<Expiration>,
    }
```
**title**: The title for the proposal.
**description**: A description on what is being proposed.
**msgs**: A vector of [CosmosMsg](https://docs.rs/cosmwasm-std/latest/cosmwasm_std/enum.CosmosMsg.html) containing all the messages to be executed in case the proposal passes. 
**latest**: Optional expiration for the proposal. If not specified, the **max_voting_period** specified at instantiation is used. 

2. Vote: Casts a vote on the specified proposal. Only available to the voter set.

```
 Vote {
        proposal_id: u64,
        vote: Vote,
    }
```
**proposal_id**: The Id of the proposal to vote on.
**vote**: The vote to cast on the proposal. 
```

pub enum Vote {
    /// Marks support for the proposal.
    Yes,
    /// Marks opposition to the proposal.
    No,
    /// Marks participation but does not count towards the ratio of support / opposed
    Abstain,
    /// Veto is generally to be treated as a No vote. Some implementations may allow certain
    /// voters to be able to Veto, or them to be counted stronger than No in some way.
    Veto,
}
```
3. Execute: Executes the messages of the proposal that has passed. Executable by any address.
```
Execute {
        proposal_id: u64,
    },
```
**proposal_id**: The Id of the proposal to execute the messages for.

4. Close: Closes a proposal in case it has expired and did not pass. 
```
Close {
        proposal_id: u64,
    },
```
**proposal_id**: The Id of the proposal to close. 


**Query Messages (What are the messages that can be queried, what does each return)**

1. Threshold: Returns the set threshold for proposal to be considered passing.
```
Threshold {},
```
2. Proposal: Returns the proposal information for the specified proposal Id. 
```
Proposal { proposal_id: u64 },
```
**proposal_id**: The Id of the proposal to get the information for.

3. ListProposals: Returns the proposal information for multiple proposals at a time. Uses pagination to specify which proposals to list.
```
ListProposals {
        start_after: Option<u64>,
        limit: Option<u32>,
    }
```
**start_after**: Optional proposal Id to start after. If 3 is specified for example, then proposal with Ids greater than 3 will be fetched.
**limit**: Optional limit to the number of proposals to fetch information for. Defaults to 10 and can be set to a maximum of 30.

4. ReverseProposals: Returns the proposal information for multiple proposals at a time. Uses pagination to specify which proposals to list. Similar to ListProposals, but fetches the proposals before a specified Id instead of after.
 ```
ReverseProposals {
        start_before: Option<u64>,
        limit: Option<u32>,
    },
```
**start_before**: Optional proposal Id to start before. If 3 is specified for example, then proposal with Ids less than 3 will be fetched.
**limit**: Optional limit to the number of proposals to fetch information for. Defaults to 10 and can be set to a maximum of 30.

5. Vote: Returns the vote of the specified address for the specified proposal.
```
Vote { proposal_id: u64, voter: String },
```
**proposal_id**: The Id of the proposal to get the vote for.
**voter**: The address of the voter to get the vote for.

6. ListVotes: Returns the voting information for the specified proposal for multiple voters at a time. Uses pagination to specify which votes to list.
```
 ListVotes {
        proposal_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    }
```
**proposal_id**: The Id of the proposal to get voting information for. 
**start_after**: Optional address to start after. 
**limit**: Optional limit to the number of Votes to fetch. Defaults to 10 and can be set to a maximum of 30.

7. Voter: Returns the weight of the specified voter.
```
 Voter { address: String },
```
**address**: The address of the voter to get the weight for.

8. ListVoters: Returns the address and weight of the voters for the ADO. Can be used with pagination.
```
ListVoters {
        start_after: Option<String>,
        limit: Option<u32>,
    },
```
**start_after**: Optional address to start after.
**limit**:Optional limit to specify the number of Voters returned. Defaults to 10 and can be set to a maximum of 30.
