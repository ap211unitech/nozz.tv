use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::Token2022,
    token_interface::{self, Mint, TokenAccount, TransferChecked},
};

use crate::{
    error::NozzError,
    events::Staked,
    state::{StakePool, StakePosition},
    utils::{calculate_pending_rewards, update_reward_per_token},
    CREATOR_TOKEN_MINT_DECIMALS,
};

pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    require!(amount > 0, NozzError::ZeroAmount);

    let clock = Clock::get()?;
    let token_mint = ctx.accounts.mint.key();
    let staker_key = ctx.accounts.staker.key();

    // Update global accumulator before any state change
    let pool = &ctx.accounts.stake_pool;
    let updated_reward_per_token = update_reward_per_token(
        pool.reward_per_token_stored,
        pool.last_update_time,
        pool.reward_end_time,
        pool.reward_rate_per_second,
        pool.total_staked,
        clock.unix_timestamp,
    )?;

    // Calculate pending rewards for user before adding new stake
    let position = &ctx.accounts.stake_position;
    let pending_rewards = calculate_pending_rewards(
        position.amount_staked,
        updated_reward_per_token,
        position.reward_per_token_paid,
        position.rewards_earned,
    )?;

    // Transfer tokens: staker ATA -> stake vault
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.staker_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
                authority: ctx.accounts.staker.to_account_info(),
            },
        ),
        amount,
        CREATOR_TOKEN_MINT_DECIMALS,
    )?;

    // Update stake pool
    let pool = &mut ctx.accounts.stake_pool;
    pool.reward_per_token_stored = updated_reward_per_token;
    pool.last_update_time = clock.unix_timestamp;
    pool.total_staked = pool
        .total_staked
        .checked_add(amount)
        .ok_or(NozzError::MathOverflow)?;

    // Update stake position
    let new_amount_staked = ctx
        .accounts
        .stake_position
        .amount_staked
        .checked_add(amount)
        .ok_or(NozzError::MathOverflow)?;

    let is_subscribed = new_amount_staked >= ctx.accounts.stake_pool.min_stake_amount;

    ctx.accounts.stake_position.set_inner(StakePosition {
        owner: staker_key,
        mint: token_mint,
        amount_staked: new_amount_staked,
        reward_per_token_paid: updated_reward_per_token,
        rewards_earned: pending_rewards,
        is_subscribed,
        staked_at: ctx
            .accounts
            .stake_position
            .staked_at
            .max(clock.unix_timestamp),
        bump: ctx.bumps.stake_position,
    });

    emit!(Staked {
        mint: token_mint,
        staker: staker_key,
        amount,
        total_staked: new_amount_staked,
        pool_total_staked: ctx.accounts.stake_pool.total_staked,
        is_subscribed,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Staked {} tokens | total: {} | subscribed: {}",
        amount,
        new_amount_staked,
        is_subscribed
    );

    Ok(())
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    /// Token-2022 mint
    #[account(
        mint::decimals = CREATOR_TOKEN_MINT_DECIMALS,
        extensions::metadata_pointer::metadata_address = mint,
        constraint = mint.key() == stake_pool.mint @ NozzError::InvalidTokenMint,
     )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [StakePool::SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// Staker's token account — tokens come from here
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = staker,
        associated_token::token_program = token_program,
    )]
    pub staker_ata: InterfaceAccount<'info, TokenAccount>,

    /// Vault that holds all staked tokens
    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = mint,
        associated_token::authority = stake_pool,
        associated_token::token_program = token_program,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// Per-user stake position — created on first stake
    #[account(
        init_if_needed,
        payer = staker,
        space = StakePosition::LEN,
        seeds = [StakePosition::SEED, mint.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,

    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
