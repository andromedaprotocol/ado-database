**ADO Summary - What is the goal of this ADO and how does it function?**
The goal of the Wrapped CW721 ADO is to create a smart contract that allows users to wrap their existing CW721 tokens. This wrapped version of the token can be used to leverage additional functionalities, such as enabling TransferAgreement without the need for a marketplace or escrow service. The primary function of this ADO is to allow users to deposit their CW721 tokens and receive a corresponding wrapped token in return. These wrapped tokens can be traded or transferred, and at any time, the holder of a wrapped token can unwrap it to get back the original CW721 token, provided the contract creator allows unwrapping. This functionality enables a variety of use cases, such as creating temporary representations of NFTs for specific purposes or adding layers of functionality to existing NFTs.

**Does it need to work with another ADO or is it standalone? Also, does it implement any modules?**
This ADO can work standalone but can also be integrated with other ADOs like Cw721 to facilitate trades without the need for a marketplace or escrow service. It does not need to implement modules like Addresslist or Rates.

**Are you planning to build this ADO yourself, or by the Andromeda team**
I am planning to build this ADO myself.

**Credits/Associations - Is this ADO based upon a previous project or ADO or in partnership with any other groups or developers? If so, please list here and provide a link if possible.**
This ADO is inspired by existing Wrapped NFT implementations and functionalities. It could also draw from other token wrapping projects in the blockchain ecosystem.

**Can you provide any docs/articles/research that explains the main idea of the ADO and how/why it is used.**
- CW721 Documentation(https://docs.andromedaprotocol.io/andromeda/andromeda-digital-objects/cw721)
- https://docs.ethos.wiki/ethereansos-docs/items/items/item-v2-protocol-overview/wrapped-items/erc721-wrapper
- https://kaigani.medium.com/wrapped-nfts-the-next-phase-in-crypto-collectibles-8253feeaabba

**ADO Flow Breakdown - Please list and provide descriptions of each step in the ADO flow sequence (show us how to work with the ADO and associated workflow, visuals are great here):**

Instantiation - What is defined when instantiating the ADO:
- Define the Owner: The owner of the ADO is specified during instantiation, providing administrative control over the contract's settings and operations.
- Link to CW721: Store the CW721 contract address that this wrapping contract will interact with.
- Set Wrapping Parameters: Define the default wrapping and unwrapping conditions. This configuration sets the foundation for how the tokens will be wrapped and unwrapped.

Execution - After instantiation, what is the process for working with the ADO:
- Wrap Token: Users send or deposit a CW721 token into the contract and receive a wrapped version of the token.
- Unwrap Token: If allowed, users can deposit the wrapped token back into the contract to receive the original CW721 token.

Queries - What type of information will you need to include, search upon:
- Check original token details: Query the contract to get details about the original CW721 token associated with a wrapped token.
- Check wrapped token details: Retrieve information about the wrapped token, including its ID and associated original token.
- Check unwrappable status: Determine if a wrapped token can be unwrapped.
- Get wrapped token count: Retrieve the total number of wrapped tokens.
- Get wrapped token address: Obtain the address of the wrapped token contract.
- Get contract's owner.

**Considerations/Concerns - What factors should be considered to mitigate risk of misuse, abuse, or other unintended scenarios, if any?**
- Data Integrity and Tamper-Proofing: Ensure that the contract logic correctly verifies the wrapping and unwrapping processes. Implement immutable data structures for storing token details to prevent unauthorized changes. Use strict validation checks to prevent malicious attempts to alter token states or conditions.
- Access Control: Implement robust access control mechanisms to restrict who can wrap and unwrap tokens. Ensure that only authorized entities can perform these actions.
- Wrapping Parameters: Define clear conditions for wrapping and unwrapping tokens, and enforce these rules strictly to ensure the system's integrity.
- Contract Upgradability and Maintenance: Consider the potential need for contract upgrades or maintenance. Implement a transparent process for updating the contract, with safeguards to prevent unauthorized changes.

**Possible Next Iterations/Future Work - How can this ADO be further enhanced?**
- Enhanced Access Control: Implement more granular access control mechanisms, such as multi-signature authorization for wrapping and unwrapping tokens.
- Dynamic Wrapping Adjustments: Introduce functionality to modify wrapping conditions under specific scenarios, with appropriate safeguards to prevent abuse.
- Automated Wrapping Mechanisms: Develop automated mechanisms to facilitate wrapping and unwrapping processes, reducing the need for manual intervention.

**Any Dependencies or Third Party Integrations? (Ex. Will this ADO need to work with anything off chain, a different app, etc?):**
- CW721 Standard Contract: Dependency on an existing CW721 standard contract to manage the minting, transfer, and ownership of NFTs. Ensure seamless interaction with the CW721 contract by correctly implementing the CW721 interface and handling related messages.
- User Interface Applications: Dependency on a user-friendly interface for interacting with the wrapping contract. Integration with web or mobile applications using frameworks like React, Vue, or Flutter to provide an intuitive interface for users to manage their wrapped NFTs.
