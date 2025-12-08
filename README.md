# MultiDistribute Solana Program

MultiDistribute is a Solana program built with Anchor that lets you create token collections and distribute rewards proportionally to users who deposit tokens.

## Program Functionality

- **Collections:**
  An authority initializes a collection with a cap on the total tokens that can be deposited, usually based on the circulating amount of the token. The authority also sets up distributions of other tokens. Users commit tokens to the collection and receive replacement tokens in return as well as their share from the distributions. The collection can be configured to either store the committed tokens in a vault or burn them.

- **Distributions:**
  The collection authority can set up a distribution associated with a collection. The distribution holds tokens that are later shared among the users who commit tokens into the collection.

- **Management:**
  The collection authority can withdraw tokens from the collection vault without affecting users’ reward eligibility.

## Instructions

- **init_collection** - Creates a new token collection with specified maximum deposit limit and burn configuration
- **withdraw_from_collection** - Authority withdraws tokens from collection vault
- **init_distribution** - Creates a new distribution for rewarding collection depositors
- **add_distribution_tokens** - Adds tokens to a distribution's reward pool
- **user_commit_and_claim_stateless** - User commits tokens and claims their share of distribution rewards

## Program Accounts

- **Collection** - Tracks configuration and state for a token collection including authority, total tokens collected, maximum deposit limit, vault, replacement mint, and burn configuration
- **Distribution** - Manages token distribution for a collection including total tokens deposited, mint, vault and amount distributed


## Build and Test

Initially built with Solana 1.18.26 and Anchor 0.28.0. Use `anchor test` to run the basic tests.

## License

This project is licensed under the GNU General Public License v3.0. You can find a copy of the license in the `LICENSE` file included with this project.

Alternatively, you can view the license online at [https://www.gnu.org/licenses/gpl-3.0.html](https://www.gnu.org/licenses/gpl-3.0.html).
