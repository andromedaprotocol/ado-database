# ADO Purpose
With the CW721 Timelock, you can lock an NFT (CW721) with a contract for a certain amount of time (currently between one day & one year). Once the timelock has expired, anyone can call the claim function to send the NFT to the defined recipient. Each locked NFT has a specific lock ID comprising the CW721 contract address concatenated with the token_id.

### Messages

***Instantiation (What is specified and stored at instantiation)***
```
pub struct InstantiateMsg {
    pub kernel_address: String,
    pub owner: Option<String>,
    pub authorized_token_addresses: Option<Vec<AndrAddr>>,
}
```

**authorized_token_addresses**: An optional vector of addresses that are authorized to interact with the contract. If not specified, any address can interact.



***Execute Messages (What are the messages that can be executed)***

1. **ReceiveNft**: Handles the receipt of an NFT and locks it for a specified duration.

```
ReceiveNft(Cw721ReceiveMsg),
```
**Cw721ReceiveMsg**: The message received when an NFT is sent to this contract. This message includes the sender, the token ID, and a message for further handling.

2. **ClaimNft**: Allows the recipient to claim the NFT once the lock period has expired.

```
ClaimNft {
    cw721_contract: AndrAddr,
    token_id: String,
},
```
**cw721_contract**: The address of the CW721 contract.
token_id: The ID of the token to be claimed.



***Query Messages (What are the messages that can be queried, what does each return)***

1. **UnlockTime**: Returns the unlock time for a specified NFT.
```
UnlockTime {
    cw721_contract: AndrAddr,
    token_id: String,
},
```

**cw721_contract**: The address of the CW721 contract.
token_id: The ID of the token.

**Returns**:
```
pub struct UnlockTimeResponse {
    pub unlock_time: u64,
}
```
**unlock_time**: The time at which the NFT can be claimed.

2. **NftDetails**: Returns the details of a locked NFT including the unlock time and the recipient address.
```
NftDetails {
    cw721_contract: AndrAddr,
    token_id: String,
},
```
**cw721_contract**: The address of the CW721 contract.
token_id: The ID of the token.

**Returns**:
```
pub struct NftDetailsResponse {
    pub unlock_time: u64,
    pub recipient: Addr,
}
```
**unlock_time**: The time at which the NFT can be claimed.
recipient: The address of the recipient who can claim the NFT after the unlock time.

### State
The contract maintains the following state:
```
pub struct TimelockInfo {
    pub unlock_time: MillisecondsExpiration,
    pub recipient: Addr,
}

pub const TIMELOCKS: Map<&str, TimelockInfo> = Map::new("timelocks");
```
**TimelockInfo**: Structure holding the unlock time and the recipient address for each locked NFT.
**TIMELOCKS**: A mapping from token IDs to their respective TimelockInfo.

This state ensures that each NFT has its own lock period and designated recipient.
