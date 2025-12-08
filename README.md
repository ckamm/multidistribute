# MultiDistribute Solana Program

MultiDistribute is a Solana program built with Anchor that lets you create token collections and distribute rewards proportionally to users who deposit tokens.

It allows an authority to set up a pool (collection) where users can commit tokens. Later, tokens from one or more distributions are shared among users based on how much they deposited. There is no cutoff for users committing or claiming tokens.

## Program Functionality

- **Collections:**
  An authority initializes a collection with a cap on the total tokens that can be deposited, usually based on the circulating amount of the token. Users commit tokens to the collection and receive replacement tokens in return. The collection can be configured to either store the committed tokens in a vault or burn them. User deposits are tracked for future reward distributions.

- **Distributions:**
  The collection authority can set up a distribution associated with a collection. The distribution holds tokens that are later shared among the users who deposited tokens into the collection.

- **Deposits & Claims:**
  Users deposit (commit) tokens into the collection to become eligible for rewards. When a distribution is available, users can claim a share of its tokens proportionally based on their deposit relative to the collection’s maximum allowed tokens.

- **Management:**
  The collection authority can adjust the maximum deposit cap and withdraw tokens from the collection vault without affecting users’ reward eligibility.

## Instructions

- **init_collection** - Creates a new token collection with specified maximum deposit limit and burn configuration
- **decrease_collection_max_collectable_tokens** - Authority reduces the maximum deposit limit for a collection
- **withdraw_from_collection** - Authority withdraws tokens from collection vault
- **init_distribution** - Creates a new distribution for rewarding collection depositors
- **add_distribution_tokens** - Adds tokens to a distribution's reward pool
- **user_commit_to_collection** - User deposits tokens into a collection and receives freshly minted replacement tokens
- **user_claim_from_distribution** - User claims their share of distribution rewards

## Program Accounts

- **Collection** - Tracks configuration and state for a token collection including authority, total tokens collected, maximum deposit limit, vault, replacement mint, and burn configuration
- **CollectionUserState** - Records how many tokens a user has deposited into a collection
- **Distribution** - Manages token distribution for a collection including total tokens deposited, mint, vault and amount distributed
- **DistributionUserState** - Tracks how many tokens a user has received from a distribution


## Build and Test

Initially built with Solana 1.18.26 and Anchor 0.28.0. Use `anchor test` to run the basic tests.

## License

This project is licensed under the GNU General Public License v3.0. You can find a copy of the license in the `LICENSE` file included with this project.

Alternatively, you can view the license online at [https://www.gnu.org/licenses/gpl-3.0.html](https://www.gnu.org/licenses/gpl-3.0.html).
