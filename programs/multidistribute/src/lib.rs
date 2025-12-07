use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
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
    /// and enforces a maximum cap.
    ///
    /// # Arguments
    /// * `counter` - Unique counter value to allow multiple collections for the same mint
    /// * `max_collectable_tokens` - Maximum number of tokens that can be deposited into this collection
    pub fn init_collection(
        ctx: Context<InitCollection>,
        counter: u64,
        max_collectable_tokens: u64,
    ) -> Result<()> {
        require!(
            max_collectable_tokens > 0,
            ErrorCode::InvalidMaxCollectableTokens
        );

        let collection = &mut ctx.accounts.collection;
        collection.authority = ctx.accounts.authority.key();
        collection.lifetime_tokens_collected = 0;
        collection.max_collectable_tokens = max_collectable_tokens;
        collection.mint = ctx.accounts.mint.key();
        collection.vault = ctx.accounts.vault.key();
        collection.replacement_mint = ctx.accounts.replacement_mint.key();
        collection.setup_done = 0;
        collection.bump = *ctx.bumps.get("collection").unwrap();
        collection.counter = counter;
        Ok(())
    }

    /// Finishes collection setup.
    pub fn finalize_collection_setup(ctx: Context<FinalizeCollectionSetup>) -> Result<()> {
        let collection = &mut ctx.accounts.collection;
        require_eq!(
            collection.setup_done,
            0,
            ErrorCode::CollectionSetupAlreadyDone
        );
        collection.setup_done = 1;
        Ok(())
    }

    /// Withdraws all tokens from the collection vault to the authority's token account.
    ///
    /// Can only be called by the collection authority. This does not affect users'
    /// deposited amounts or their ability to receive from distributions.
    pub fn withdraw_from_collection(ctx: Context<WithdrawFromCollection>) -> Result<()> {
        let collection = &ctx.accounts.collection;

        // Transfer tokens from collection vault to authority
        let counter_bytes = collection.counter.to_le_bytes();
        let authority_seeds = &[
            b"collection",
            collection.authority.as_ref(),
            collection.mint.as_ref(),
            &counter_bytes,
            &[collection.bump],
        ];
        let signer = &[&authority_seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.authority_token_account.to_account_info(),
                authority: ctx.accounts.collection.to_account_info(),
            },
            signer,
        );
        token::transfer(transfer_ctx, ctx.accounts.vault.amount)?;

        Ok(())
    }

    /// Initializes a new distribution associated with a collection.
    ///
    /// A distribution allows proportional sharing of tokens to collection depositors.
    /// The distributed token type can be different from the collected token type.
    /// Can only be called by the collection authority.
    pub fn init_distribution(ctx: Context<InitDistribution>) -> Result<()> {
        require_eq!(
            ctx.accounts.collection.setup_done,
            0,
            ErrorCode::CollectionSetupAlreadyDone
        );

        let distribution = &mut ctx.accounts.distribution;
        distribution.collection = ctx.accounts.collection.key();
        distribution.lifetime_deposited_tokens = 0;
        distribution.mint = ctx.accounts.mint.key();
        distribution.vault = ctx.accounts.vault.key();
        distribution.distributed_tokens = 0;
        distribution.bump = *ctx.bumps.get("distribution").unwrap();
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
        require_eq!(
            ctx.accounts.collection.setup_done,
            0,
            ErrorCode::CollectionSetupAlreadyDone
        );

        let distribution = &mut ctx.accounts.distribution;

        // Transfer tokens to the distribution vault
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.authority_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, amount)?;

        distribution.lifetime_deposited_tokens = distribution
            .lifetime_deposited_tokens
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;

        Ok(())
    }

    /// Commits tokens to a collection's vault.
    ///
    /// Users commit tokens to become eligible for distributions. Their share of
    /// future distributions will be proportional to their committed amount relative
    /// to the collection's max_collectable_tokens. Users receive freshly minted
    /// replacement tokens equal to their committed amount.
    ///
    /// # Arguments
    /// * `amount` - Number of tokens to commit to the collection
    pub fn user_commit_to_collection(
        ctx: Context<UserCommitToCollection>,
        _counter: u64, // used for seed derivation in context struct
        amount: u64,
    ) -> Result<()> {
        let collection = &ctx.accounts.collection;
        require_eq!(
            ctx.accounts.collection.setup_done,
            1,
            ErrorCode::CollectionSetupNotDone
        );

        let user_state = &mut ctx.accounts.user_state;

        // Transfer the committed tokens
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, amount)?;

        // Mint replacement tokens to user
        let counter_bytes = collection.counter.to_le_bytes();
        let seeds = &[
            b"collection",
            collection.authority.as_ref(),
            collection.mint.as_ref(),
            &counter_bytes,
            &[collection.bump],
        ];
        let signer = &[&seeds[..]];

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
            signer,
        );
        token::mint_to(mint_ctx, amount)?;

        // Update states
        let collection = &mut ctx.accounts.collection;
        collection.lifetime_tokens_collected = collection
            .lifetime_tokens_collected
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        user_state.user = ctx.accounts.user.key();
        user_state.deposited_amount = amount;

        require!(
            collection.lifetime_tokens_collected <= collection.max_collectable_tokens,
            ErrorCode::MaxCollectableTokensExceeded
        );

        Ok(())
    }

    /// Claims a user's share of tokens from a distribution.
    ///
    /// The amount claimed is proportional to the user's deposit in the collection
    /// relative to the collection's max_collectable_tokens. Can be called multiple
    /// times as more tokens are added to the distribution.
    pub fn user_claim_from_distribution(ctx: Context<UserClaimFromDistribution>) -> Result<()> {
        let collection = &ctx.accounts.collection;
        require_eq!(
            ctx.accounts.collection.setup_done,
            1,
            ErrorCode::CollectionSetupNotDone
        );

        let distribution = &ctx.accounts.distribution;
        let collection_user_state = &ctx.accounts.collection_user_state;
        let distribution_user_state = &mut ctx.accounts.distribution_user_state;

        // Calculate user's share:
        // Since user_claim_from_distribution() can be called at any time, in particular before all
        // users have deposited, the fixed max_collectable_tokens denominator is used.
        // That means that if less than max_collectable_tokens end up deposited, a large
        // part of the distribution may not be handed out.
        let user_share = (collection_user_state.deposited_amount as u128)
            .checked_mul(distribution.lifetime_deposited_tokens as u128)
            .ok_or(ErrorCode::Overflow)?
            // Integer division rounds down, ensuring we never overpay users
            .checked_div(collection.max_collectable_tokens as u128)
            .ok_or(ErrorCode::Overflow)? as u64;

        let amount_to_receive = user_share;

        // Note that amount_to_receive may be zero. That is ok, the instruction
        // should nevertheless succeed.

        // Transfer tokens from distribution vault to user
        let authority_seeds = &[
            b"distribution",
            distribution.collection.as_ref(),
            distribution.mint.as_ref(),
            &[distribution.bump],
        ];
        let signer = &[&authority_seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.distribution_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.distribution.to_account_info(),
            },
            signer,
        );
        token::transfer(transfer_ctx, amount_to_receive)?;

        // Update states
        let distribution = &mut ctx.accounts.distribution;
        distribution_user_state.received_amount = user_share;
        distribution.distributed_tokens = distribution
            .distributed_tokens
            .checked_add(amount_to_receive)
            .ok_or(ErrorCode::Overflow)?;

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
    pub collection: Account<'info, Collection>,

    /// The SPL token mint for tokens being collected
    pub mint: Account<'info, Mint>,

    /// Associated token account owned by the collection PDA that holds deposited tokens
    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = collection
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The replacement mint owned by the collection
    #[account(
        init,
        payer = authority,
        mint::decimals = mint.decimals,
        mint::authority = collection,
        seeds = [
            b"replacement_mint",
            collection.key().as_ref()
        ],
        bump
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
    #[account(
        has_one = authority
    )]
    pub collection: Account<'info, Collection>,

    /// The collection's vault, holding the tokens to withdraw
    #[account(
        mut,
        address = collection.vault
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The token account to receive the withdrawn tokens
    #[account(
        mut,
        token::mint = vault.mint
    )]
    pub authority_token_account: Account<'info, TokenAccount>,

    /// The authority of the collection
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct FinalizeCollectionSetup<'info> {
    /// The collection to finish setup for
    #[account(
        mut,
        has_one = authority
    )]
    pub collection: Account<'info, Collection>,

    /// The authority of the collection
    pub authority: Signer<'info>,
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
    #[account(
        has_one = authority
    )]
    pub collection: Account<'info, Collection>,

    /// The SPL token mint for tokens being distributed. Can be the same as or
    /// different from the collection's mint.
    pub mint: Account<'info, Mint>,

    /// Associated token account owned by the distribution PDA that holds tokens to distribute
    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = distribution
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The collection's authority and payer for the distribution accounts
    #[account(mut)]
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
    #[account(
        mut,
        has_one = collection
    )]
    pub distribution: Account<'info, Distribution>,

    /// The collection this distribution is associated with
    pub collection: Account<'info, Collection>,

    /// The distribution's vault to receive the tokens
    #[account(
        mut,
        constraint = vault.key() == distribution.vault
    )]
    pub vault: Account<'info, TokenAccount>,

    /// The token account providing the tokens to distribute
    #[account(
        mut,
        constraint = authority_token_account.mint == vault.mint
    )]
    pub authority_token_account: Account<'info, TokenAccount>,

    /// The signer who owns the token account providing the tokens
    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(counter: u64)]
pub struct UserCommitToCollection<'info> {
    /// The collection to commit tokens to
    #[account(mut)]
    pub collection: Account<'info, Collection>,

    /// PDA tracking this user's deposits to this collection
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<CollectionUserState>(),
        seeds = [
            b"user_state",
            collection.key().as_ref(),
            user.key().as_ref(),
            counter.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub user_state: Account<'info, CollectionUserState>,

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

    /// The user depositing tokens, potentially paying for the user_state account
    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UserClaimFromDistribution<'info> {
    /// The collection associated with this distribution
    pub collection: Account<'info, Collection>,

    /// The distribution to claim tokens from
    #[account(
        mut,
        has_one = collection
    )]
    pub distribution: Account<'info, Distribution>,

    /// The user's state for the collection, tracking their deposits
    #[account(
        has_one = user
    )]
    pub collection_user_state: Account<'info, CollectionUserState>,

    /// PDA tracking how many tokens this user has claimed from this distribution for the
    /// collection state
    #[account(
        init,
        payer = user,
        space = 8 + size_of::<DistributionUserState>(),
        seeds = [
            b"distribution_user_state",
            distribution.key().as_ref(),
            collection_user_state.key().as_ref(),
        ],
        bump
    )]
    pub distribution_user_state: Account<'info, DistributionUserState>,

    /// The vault holding the tokens to be distributed
    #[account(
        mut,
        address = distribution.vault
    )]
    pub distribution_vault: Account<'info, TokenAccount>,

    /// The user's associated token account to receive the claimed tokens
    #[account(
        mut,
        associated_token::mint = distribution.mint,
        associated_token::authority = user
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// The user claiming tokens from the distribution
    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

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
    pub setup_done: u8,
    pub bump: u8,
    pub counter: u64,
}

/// Tracks an individual user's deposits into a collection.
/// Used to calculate their share of distributions.
#[account]
pub struct CollectionUserState {
    pub user: Pubkey,
    pub deposited_amount: u64,
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

/// Tracks how many tokens a user has received from a specific distribution.
/// Prevents double-claiming and enables partial claims as more tokens are added.
#[account]
pub struct DistributionUserState {
    pub received_amount: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Arithmetic overflow in calculation")]
    Overflow,

    #[msg("The collection isn't fully initialized yet")]
    CollectionSetupNotDone,

    #[msg("The collection setup is already completed")]
    CollectionSetupAlreadyDone,

    #[msg("Tokens for the collection exceed configured maximum")]
    MaxCollectableTokensExceeded,

    #[msg("New maximum tokens must be greater than or equal to total tokens")]
    MaxCollectableTokensBelowTotal,

    #[msg("Maximum collectable tokens must be greater than zero")]
    InvalidMaxCollectableTokens,

    #[msg("New maximum tokens must be less than current maximum")]
    InvalidDecrease,
}
