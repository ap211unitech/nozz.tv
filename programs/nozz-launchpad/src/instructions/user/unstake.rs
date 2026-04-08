use anchor_lang::prelude::*;

use anchor_spl::{
    token_2022::Token2022,
    token_interface::{self, Mint, TokenAccount, TransferChecked},
};

use crate::{
    error::NozzError,
    events::Unstaked,
    state::{StakePool, StakePosition},
    utils::{calculate_pending_rewards, update_reward_per_token},
    CREATOR_TOKEN_MINT_DECIMALS,
};

pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
    require!(amount > 0, NozzError::ZeroAmount);
    require!(
        ctx.accounts.stake_position.amount_staked > 0,
        NozzError::NothingStaked
    );

    let clock = Clock::get()?;
    let token_mint = ctx.accounts.mint.key();
    let staker_key = ctx.accounts.staker.key();

    // Update global accumulator
    let pool = &ctx.accounts.stake_pool;
    let updated_reward_per_token = update_reward_per_token(
        pool.reward_per_token_stored,
        pool.last_update_time,
        pool.reward_end_time,
        pool.reward_rate_per_second,
        pool.total_staked,
        clock.unix_timestamp,
    )?;

    // Settle pending rewards before removing stake
    let position = &ctx.accounts.stake_position;
    let pending_rewards = calculate_pending_rewards(
        position.amount_staked,
        updated_reward_per_token,
        position.reward_per_token_paid,
        position.rewards_earned,
    )?;

    // Transfer tokens: stake vault -> staker ATA
    let sp_bump = ctx.accounts.stake_pool.bump;
    let sp_seeds: &[&[u8]] = &[StakePool::SEED, token_mint.as_ref(), &[sp_bump]];
    let signer_seeds = &[sp_seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.stake_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.staker_ata.to_account_info(),
                authority: ctx.accounts.stake_pool.to_account_info(),
            },
            signer_seeds,
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
        .checked_sub(amount)
        .ok_or(NozzError::MathOverflow)?;

    // Update stake position
    let new_amount_staked = ctx
        .accounts
        .stake_position
        .amount_staked
        .checked_sub(amount)
        .ok_or(NozzError::MathOverflow)?;

    // Subscriber status drops immediately if below threshold
    let is_subscribed = new_amount_staked >= ctx.accounts.stake_pool.min_stake_amount;

    ctx.accounts.stake_position.amount_staked = new_amount_staked;
    ctx.accounts.stake_position.reward_per_token_paid = updated_reward_per_token;
    ctx.accounts.stake_position.rewards_earned = pending_rewards;
    ctx.accounts.stake_position.is_subscribed = is_subscribed;

    emit!(Unstaked {
        mint: token_mint,
        staker: staker_key,
        amount,
        total_staked: new_amount_staked,
        pool_total_staked: ctx.accounts.stake_pool.total_staked,
        is_subscribed,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Unstaked {} tokens | remaining: {} | subscribed: {}",
        amount,
        new_amount_staked,
        is_subscribed
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

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

    /// Vault holding staked tokens — signs via stake_pool PDA
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = stake_pool,
        associated_token::token_program = token_program,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// Staker receives tokens here
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = staker,
        associated_token::token_program = token_program,
    )]
    pub staker_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [StakePosition::SEED, mint.key().as_ref(), staker.key().as_ref()],
        bump = stake_position.bump,
        constraint = stake_position.owner == staker.key() @ NozzError::UnAuthorized,
        constraint = stake_position.amount_staked >= amount @ NozzError::InsufficientStakedBalance,
    )]
    pub stake_position: Account<'info, StakePosition>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}
