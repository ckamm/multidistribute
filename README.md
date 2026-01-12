# MultiDistribute Solana Program

MultiDistribute is a Solana program built with Anchor that lets you create token collections and distribute rewards proportionally to users who deposit tokens.

## Program Functionality

- **Collections:**
  An authority initializes a collection with a cap on the total tokens that can be deposited, usually based on the circulating amount of the token. The authority also sets up distributions of other tokens. Users commit tokens to the collection and receive replacement tokens in return as well as their share from the distributions. The collection can be configured to either store the committed tokens in a vault or burn them.

  Each collection includes a `terms_hash` (32 bytes) set at initialization. When users claim, they must provide the same hash, creating on-chain proof of their agreement to the associated terms of service.

- **Distributions:**
  The collection authority can set up a distribution associated with a collection. The distribution holds tokens that are later shared among the users who commit tokens into the collection.

- **Management:**
  The collection authority can withdraw tokens from the collection vault without affecting users’ reward eligibility.

## Instructions

- **init_collection** - Creates a new token collection with specified maximum deposit limit, burn configuration, and terms hash
- **withdraw_from_collection** - Authority withdraws tokens from collection vault (or burns them if `burn_instead_of_withdraw` was set)
- **init_distribution** - Creates a new distribution for rewarding collection depositors
- **add_distribution_tokens** - Adds tokens to a distribution's reward pool
- **user_commit_and_claim_stateless** - User commits tokens and claims their share of distribution rewards (must provide matching terms hash)

## Program Accounts

- **Collection** - Tracks configuration and state for a token collection including authority, total tokens collected, maximum deposit limit, vault, replacement mint, `burn_on_deposit` flag, `burn_instead_of_withdraw` flag, and `terms_hash`
- **Distribution** - Manages token distribution for a collection including total tokens deposited, mint, vault and amount distributed

## Build and Test

Built with Solana 2.2.15 and Anchor 0.30.1.

- Build:`anchor build --no-idl`
- Create IDL:`RUSTUP_TOOLCHAIN=nightly-2024-12-31 anchor idl build -o ../../target/idl/multidistribute.json -t ../../target/types/multidistribute.ts` in `programs/multidistribute/`.
- Run tests:`anchor test --skip-build`.
- Verifiable build `solana-verify build -b solanafoundation/solana-verifiable-build:2.2.15`, placed into target/deploy/


## Risks

With an immutable program and burn_on_deposit=true, the program should be safe against a malicious authority.
Otherwise both the program upgrade authority and the collection authority must be trusted.

- The program upgrade authority is in control of all the funds, both deposited tokens and to-be-distributed tokens.
- If the deposited tokens aren't burned immediately, the collection authority could withdraw them and deposit them again in a loop, draining to-be-distributed tokens. Setting `burn_instead_of_withdraw=true` mitigates this risk by ensuring the authority can only burn (not withdraw and reuse) tokens from the vault.
- If deposited tokens are burned immediately, that mint's supply changes, which can have effects on the quorum of governance programs.
- If tokens are added to a distribution late, after some users have already claimed, these users have no way of receiving the added tokens. Don't add tokens late.

## License

This project is licensed under the GNU General Public License v3.0. You can find a copy of the license in the `LICENSE` file included with this project.

Alternatively, you can view the license online at [https://www.gnu.org/licenses/gpl-3.0.html](https://www.gnu.org/licenses/gpl-3.0.html).
