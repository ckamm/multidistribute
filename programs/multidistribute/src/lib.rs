use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use std::mem::size_of;

declare_id!("DisJzzeTrLXzJgtaaqBxcNKLrLFyc4mY3ELGCdEkVPzt");

#[program]
pub mod multidistribute {
    use super::*;

    /// Initializes a new collection for gathering tokens from users.
    ///
    /// A collection allows users to deposit tokens and receive proportional rewards
    /// from multiple distributions. The collection tracks the total tokens deposited
    /// and enforces a maximum cap based on the mint's current supply.
    ///
    /// # Arguments
    /// * `counter` - Unique counter value to allow multiple collections for the same mint
    /// * `burn_on_deposit` - If true, committed tokens will be burned instead of stored in the vault
    /// * `burn_instead_of_withdraw` - If true, withdraw_from_collection burns tokens instead of withdrawing
    pub fn init_collection(
        ctx: Context<InitCollection>,
        counter: u64,
        burn_on_deposit: bool,
        burn_instead_of_withdraw: bool,
    ) -> Result<()> {
        let max_collectable_tokens = ctx.accounts.mint.supply;
        require!(
            max_collectable_tokens > 0,
            ErrorCode::InvalidMaxCollectableTokens
        );

        // Transfer mint authority from the authority signer to the collection PDA
        let set_authority_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                current_authority: ctx.accounts.authority.to_account_info(),
                account_or_mint: ctx.accounts.replacement_mint.to_account_info(),
            },
        );
        token::set_authority(
            set_authority_ctx,
            token::spl_token::instruction::AuthorityType::MintTokens,
            Some(ctx.accounts.collection.key()),
        )?;

        let collection = &mut ctx.accounts.collection;
        collection.authority = ctx.accounts.authority.key();
        collection.lifetime_tokens_collected = 0;
        collection.max_collectable_tokens = max_collectable_tokens;
        collection.mint = ctx.accounts.mint.key();
        collection.vault = ctx.accounts.vault.key();
        collection.replacement_mint = ctx.accounts.replacement_mint.key();
        collection.bump = *ctx.bumps.get("collection").unwrap();
        collection.counter = counter;
        collection.burn_on_deposit = burn_on_deposit;
        collection.burn_instead_of_withdraw = burn_instead_of_withdraw;
        collection.num_distributions = 0;
        collection.distributions = [Pubkey::default(); MAX_DISTRIBUTIONS];
        Ok(())
    }

    /// Withdraws all tokens from the collection vault to the authority's token account,
    /// or burns them if `burn_instead_of_withdraw` was set during collection initialization.
    ///
    /// Can only be called by the collection authority.
    pub fn withdraw_from_collection(ctx: Context<WithdrawFromCollection>) -> Result<()> {
        let collection = &ctx.accounts.collection;
        let amount = ctx.accounts.vault.amount;

        let counter_bytes = collection.counter.to_le_bytes();
        let authority_seeds = &[
            b"collection",
            collection.authority.as_ref(),
            collection.mint.as_ref(),
            &counter_bytes,
            &[collection.bump],
        ];
        let signer = &[&authority_seeds[..]];

        if collection.burn_instead_of_withdraw {
            // Burn tokens from the vault
            let burn_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.collection.to_account_info(),
                },
                signer,
            );
            token::burn(burn_ctx, amount)?;
        } else {
            // Transfer tokens from collection vault to authority
            let transfer_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.authority_token_account.to_account_info(),
                    authority: ctx.accounts.collection.to_account_info(),
                },
                signer,
            );
            token::transfer(transfer_ctx, amount)?;
        }

        Ok(())
    }

    /// Initializes a new distribution associated with a collection.
    ///
    /// A distribution allows proportional sharing of tokens to collection depositors.
    /// The distributed token type can be different from the collected token type.
    /// Can only be called by the collection authority.
    pub fn init_distribution(ctx: Context<InitDistribution>) -> Result<()> {
        let collection = &mut ctx.accounts.collection;

        // Check we haven't exceeded max distributions
        require!(
            (collection.num_distributions as usize) < MAX_DISTRIBUTIONS,
            ErrorCode::MaxDistributionsExceeded
        );

        let distribution = &mut ctx.accounts.distribution;
        distribution.collection = collection.key();
        distribution.lifetime_deposited_tokens = 0;
        distribution.mint = ctx.accounts.mint.key();
        distribution.vault = ctx.accounts.vault.key();
        distribution.distributed_tokens = 0;
        distribution.bump = *ctx.bumps.get("distribution").unwrap();

        // Register the distribution in the collection
        let idx = collection.num_distributions as usize;
        collection.distributions[idx] = distribution.key();
        collection.num_distributions += 1;

        Ok(())
    }

    /// Adds tokens to a distribution's vault for later distribution to users.
    ///
    /// Anyone can add tokens to a distribution. This allows for flexible token
    /// sourcing - the tokens don't have to come from the collection authority.
    ///
    /// # Arguments
    /// * `amount` - Number of tokens to add to the distribution
    pub fn add_distribution_tokens(ctx: Context<AddDistributionTokens>, amount: u64) -> Result<()> {
        let distribution = &mut ctx.accounts.distribution;

        // Transfer tokens to the distribution vault
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, amount)?;

        distribution.lifetime_deposited_tokens = distribution
            .lifetime_deposited_tokens
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;

        Ok(())
    }

    /// Commits tokens and immediately claims from multiple distributions in a single transaction.
    ///
    /// This instruction combines committing tokens to a collection with claiming from
    /// multiple distributions, leaving no on-chain state.
    ///
    /// # Arguments
    /// * `amount` - Number of tokens to commit to the collection
    ///
    /// # Remaining Accounts
    /// For each distribution to claim from, provide 3 accounts in order:
    /// - Distribution account (the Distribution PDA)
    /// - Distribution vault (the token account holding distribution tokens)
    /// - User's token account for this distribution's mint (to receive claimed tokens)
    pub fn user_commit_and_claim_stateless(
        ctx: Context<UserCommitAndClaimStateless>,
        amount: u64,
    ) -> Result<()> {
        let collection = &ctx.accounts.collection;

        // Either burn or transfer the tokens
        if collection.burn_on_deposit {
            let burn_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::burn(burn_ctx, amount)?;
        } else {
            let transfer_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::transfer(transfer_ctx, amount)?;
        }

        // Mint replacement tokens to user
        let counter_bytes = collection.counter.to_le_bytes();
        let collection_seeds = &[
            b"collection",
            collection.authority.as_ref(),
            collection.mint.as_ref(),
            &counter_bytes,
            &[collection.bump],
        ];
        let collection_signer = &[&collection_seeds[..]];

        let mint_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.replacement_mint.to_account_info(),
                to: ctx
                    .accounts
                    .user_replacement_token_account
                    .to_account_info(),
                authority: ctx.accounts.collection.to_account_info(),
            },
            collection_signer,
        );
        token::mint_to(mint_ctx, amount)?;

        // Update collection state
        let collection = &mut ctx.accounts.collection;
        collection.lifetime_tokens_collected = collection
            .lifetime_tokens_collected
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;

        require!(
            collection.lifetime_tokens_collected <= collection.max_collectable_tokens,
            ErrorCode::MaxCollectableTokensExceeded
        );

        // Process distributions from remaining accounts
        // Each distribution requires 3 accounts: distribution, distribution_vault, user_token_account
        let remaining = ctx.remaining_accounts;
        require!(
            remaining.len() % 3 == 0,
            ErrorCode::InvalidRemainingAccounts
        );

        let num_distributions = collection.num_distributions as usize;
        let registered_distributions = &collection.distributions[..num_distributions];

        // Verify the correct number of distributions are provided
        require!(
            remaining.len() / 3 == num_distributions,
            ErrorCode::DistributionsMismatch
        );

        let max_collectable_tokens = collection.max_collectable_tokens;
        let collection_key = collection.key();
        let token_program_key = ctx.accounts.token_program.key();

        for i in (0..remaining.len()).step_by(3) {
            let distribution_info = &remaining[i];
            let distribution_vault_info = &remaining[i + 1];

            // Note: Target token account is not explicitly validated:
            // The spl_token::transfer CPI verifies the account owner and token
            // mint, and the token owner can legitimately be different from
            // user.key().
            let user_dist_token_account_info = &remaining[i + 2];

            let distribution_index = i / 3;

            // Verify the distribution matches the registered one at this index
            require!(
                distribution_info.key() == registered_distributions[distribution_index],
                ErrorCode::DistributionsMismatch
            );

            // Verify distribution account is owned by this program
            // (redundant: key was already checked)
            require!(
                distribution_info.owner == &crate::ID,
                ErrorCode::InvalidDistributionOwner
            );

            // Deserialize and validate distribution
            let mut distribution_data = distribution_info.try_borrow_mut_data()?;
            let mut distribution: Distribution =
                Distribution::try_deserialize(&mut distribution_data.as_ref())?;

            // Verify distribution belongs to this collection
            // (redundant: key was already checked)
            require!(
                distribution.collection == collection_key,
                ErrorCode::DistributionCollectionMismatch
            );

            // Verify vault matches
            require!(
                distribution.vault == distribution_vault_info.key(),
                ErrorCode::InvalidDistributionVault
            );

            // Calculate user's share
            let user_share = (amount as u128)
                .checked_mul(distribution.lifetime_deposited_tokens as u128)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(max_collectable_tokens as u128)
                .ok_or(ErrorCode::Overflow)? as u64;

            // user_share == 0 is valid.

            // Copy out pubkeys needed for seeds
            let dist_collection = distribution.collection;
            let dist_mint = distribution.mint;
            let dist_bump = distribution.bump;

            // Update distribution state
            distribution.distributed_tokens = distribution
                .distributed_tokens
                .checked_add(user_share)
                .ok_or(ErrorCode::Overflow)?;

            // Serialize the updated distribution back
            distribution.try_serialize(&mut distribution_data.as_mut())?;

            // Drop the borrow before the CPI
            drop(distribution_data);

            // Transfer tokens from distribution vault to user using invoke_signed
            let distribution_seeds: &[&[u8]] = &[
                b"distribution",
                dist_collection.as_ref(),
                dist_mint.as_ref(),
                &[dist_bump],
            ];

            let transfer_ix = spl_token::instruction::transfer(
                &token_program_key,
                distribution_vault_info.key,
                user_dist_token_account_info.key,
                distribution_info.key,
                &[],
                user_share,
            )?;

            invoke_signed(
                &transfer_ix,
                &[
                    distribution_vault_info.clone(),
                    user_dist_token_account_info.clone(),
                    distribution_info.clone(),
                ],
                &[distribution_seeds],
            )?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(counter: u64)]
pub struct InitCollection<'info> {
    /// The collection PDA that is created to hold configuration and state
    #[account(
        init,
        payer = authority,
        space = 8 + size_of::<Collection>(),
        seeds = [
            b"collection",
            authority.key().as_ref(),
            mint.key().as_ref(),
            counter.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub collection: Box<Account<'info, Collection>>,

    /// The SPL token mint for tokens being collected
    pub mint: Account<'info, Mint>,

    /// Associated token account owned by the collection PDA that holds deposited tokens
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = collection
    )]
    pub vault: Account<'info, TokenAccount>,

    /// Pre-created replacement mint.
    ///
    /// Mint authority will be taken over, metadata must have been
    /// initialized in advance.
    #[account(
        mut,
        constraint = replacement_mint.supply == 0 @ ErrorCode::ReplacementMintHasSupply,
        constraint = replacement_mint.mint_authority.contains(&authority.key()) @ ErrorCode::ReplacementMintAuthorityMismatch,
        constraint = replacement_mint.freeze_authority.is_none() @ ErrorCode::ReplacementMintHasFreezeAuthority,
        constraint = replacement_mint.decimals == mint.decimals @ ErrorCode::ReplacementMintDecimalsMismatch,
    )]
    pub replacement_mint: Account<'info, Mint>,

    /// The authority who can manage this collection and pays for these accounts
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawFromCollection<'info> {
    /// The collection to withdraw from
    pub collection: Box<Account<'info, Collection>>,

    /// The SPL token mint (required for burning if burn_instead_of_withdraw is set)
    #[account(
        mut,
        address = collection.mint
    )]
    pub mint: Account<'info, Mint>,

    /// The collection's vault, holding the tokens to withdraw
    #[account(
        mut,
        address = collection.vault
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The token account to receive the withdrawn tokens (unused if burning)
    #[account(
        mut,
        token::mint = mint
    )]
    pub authority_token_account: Account<'info, TokenAccount>,

    /// The authority of the collection
    #[account(
        address = collection.authority
    )]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitDistribution<'info> {
    /// The created distribution PDA that manages token distribution
    #[account(
        init,
        payer = authority,
        space = 8 + size_of::<Distribution>(),
        seeds = [
            b"distribution",
            collection.key().as_ref(),
            mint.key().as_ref()
        ],
        bump
    )]
    pub distribution: Account<'info, Distribution>,

    /// The collection this distribution is associated with
    #[account(mut)]
    pub collection: Box<Account<'info, Collection>>,

    /// The SPL token mint for tokens being distributed. Can be the same as or
    /// different from the collection's mint.
    pub mint: Account<'info, Mint>,

    /// Associated token account owned by the distribution PDA that holds tokens to distribute
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = distribution
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The collection's authority and payer for the distribution accounts
    #[account(
        mut,
        address = collection.authority
    )]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

/// Add tokens to a distribution.
///
/// Note that _anyone_ can add tokens. This is because often the authority for
/// tokens to be distributed may not be the same as the collection authority.
#[derive(Accounts)]
pub struct AddDistributionTokens<'info> {
    /// The distribution to add tokens to
    #[account(mut)]
    pub distribution: Account<'info, Distribution>,

    /// The distribution's vault to receive the tokens
    #[account(
        mut,
        address = distribution.vault
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The token account providing the tokens to distribute
    #[account(
        mut,
        token::mint = vault.mint
        // intentionally not checking the owner: could be delegated
    )]
    pub depositor_token_account: Account<'info, TokenAccount>,

    /// The signer who owns the token account providing the tokens
    #[account(mut)]
    pub depositor: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

/// Commits tokens to a collection and claims from multiple distributions atomically,
/// without creating any on-chain user state.
///
/// Remaining accounts should be provided in groups of 3 for each distribution:
/// - Distribution account (writable)
/// - Distribution vault (writable)
/// - User's token account for the distribution mint (writable)
#[derive(Accounts)]
pub struct UserCommitAndClaimStateless<'info> {
    /// The collection to commit tokens to
    #[account(mut)]
    pub collection: Box<Account<'info, Collection>>,

    /// The SPL token mint for tokens being collected
    #[account(
        mut,
        address = collection.mint
    )]
    pub mint: Account<'info, Mint>,

    /// The token account providing the tokens to deposit
    #[account(
        mut,
        token::mint = mint
        // intentionally not checking the owner: could be delegated
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// The collection's vault to receive the deposited tokens (if not burning)
    #[account(
        mut,
        address = collection.vault
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The replacement mint owned by the collection
    #[account(
        mut,
        address = collection.replacement_mint
    )]
    pub replacement_mint: Account<'info, Mint>,

    /// The user's token account to receive replacement tokens
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = replacement_mint,
        associated_token::authority = user
    )]
    pub user_replacement_token_account: Account<'info, TokenAccount>,

    /// The user committing tokens
    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

/// Maximum number of distributions that can be registered to a collection
pub const MAX_DISTRIBUTIONS: usize = 20;

/// Tracks configuration and state for token collection.
/// Holds deposited tokens and manages distribution eligibility.
#[account]
pub struct Collection {
    pub authority: Pubkey,
    /// sum of tokens ever collected (including previously withdrawn!)
    pub lifetime_tokens_collected: u64,
    /// maximum amount of tokens depositable, used for reward share computation
    pub max_collectable_tokens: u64,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub replacement_mint: Pubkey,
    pub bump: u8,
    pub counter: u64,
    /// whether to burn input tokens instead of collecting them
    pub burn_on_deposit: bool,
    /// whether withdraw_from_collection burns instead of withdrawing
    pub burn_instead_of_withdraw: bool,
    /// number of registered distributions
    pub num_distributions: u8,
    /// registered distribution pubkeys (up to MAX_DISTRIBUTIONS)
    pub distributions: [Pubkey; MAX_DISTRIBUTIONS],
}

/// Manages token distribution to collection participants.
/// Tracks deposited tokens and handles proportional distribution based on user deposits.
#[account]
pub struct Distribution {
    pub collection: Pubkey,
    /// total tokens ever deposited into this distribution
    pub lifetime_deposited_tokens: u64,
    pub mint: Pubkey,
    pub vault: Pubkey,
    /// amount of tokens handed out to users
    pub distributed_tokens: u64,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Arithmetic overflow in calculation")]
    Overflow,

    #[msg("Tokens for the collection exceed configured maximum")]
    MaxCollectableTokensExceeded,

    #[msg("Maximum collectable tokens must be greater than zero")]
    InvalidMaxCollectableTokens,

    #[msg("Remaining accounts must be provided in groups of 3 (distribution, vault, user_token_account)")]
    InvalidRemainingAccounts,

    #[msg("Distribution does not belong to the specified collection")]
    DistributionCollectionMismatch,

    #[msg("Distribution vault does not match distribution account")]
    InvalidDistributionVault,

    #[msg("Distribution account has invalid owner")]
    InvalidDistributionOwner,

    #[msg("Maximum number of distributions exceeded")]
    MaxDistributionsExceeded,

    #[msg("Must provide all registered distributions in the correct order")]
    DistributionsMismatch,

    #[msg("Replacement mint must have zero supply")]
    ReplacementMintHasSupply,

    #[msg("Replacement mint authority must be the collection authority")]
    ReplacementMintAuthorityMismatch,

    #[msg("Replacement mint must have freeze authority disabled")]
    ReplacementMintHasFreezeAuthority,

    #[msg("Replacement mint decimals must match the collected mint")]
    ReplacementMintDecimalsMismatch,
}
