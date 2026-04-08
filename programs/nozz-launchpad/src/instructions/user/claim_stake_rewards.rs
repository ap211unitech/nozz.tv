use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::Token2022,
    token_interface::{self, Mint, TokenAccount, TransferChecked},
};

use crate::{
    CREATOR_TOKEN_MINT_DECIMALS, error::NozzError, events::RewardsClaimed, state::{StakePool, StakePosition}, utils::{calculate_pending_rewards, update_reward_per_token}
};

pub fn claim_stake_rewards(ctx: Context<ClaimStakeRewards>) -> Result<()> {
    let clock = Clock::get()?;
    let mint_key = ctx.accounts.mint.key();
    let staker_key = ctx.accounts.staker.key();

    // Update accumulator
    let pool = &ctx.accounts.stake_pool;
    let updated_reward_per_token = update_reward_per_token(
        pool.reward_per_token_stored,
        pool.last_update_time,
        pool.reward_end_time,
        pool.reward_rate_per_second,
        pool.total_staked,
        clock.unix_timestamp,
    )?;

    // Calculate total claimable
    let position = &ctx.accounts.stake_position;
    let claimable = calculate_pending_rewards(
        position.amount_staked,
        updated_reward_per_token,
        position.reward_per_token_paid,
        position.rewards_earned,
    )?;

    require!(claimable > 0, NozzError::NoRewardsToClaim);

    // Cap at vault balance (shouldn't happen in practice but defensive)
    let actual_claim = claimable.min(ctx.accounts.stake_pool.reward_vault_balance);
    require!(actual_claim > 0, NozzError::RewardPoolEmpty);

    // Transfer rewards: reward vault -> staker ATA
    let sp_bump = ctx.accounts.stake_pool.bump;
    let sp_seeds: &[&[u8]] = &[StakePool::SEED, mint_key.as_ref(), &[sp_bump]];
    let signer_seeds = &[sp_seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.stake_reward_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.staker_ata.to_account_info(),
                authority: ctx.accounts.stake_pool.to_account_info(),
            },
            signer_seeds,
        ),
        actual_claim,
        CREATOR_TOKEN_MINT_DECIMALS,
    )?;

    // Update pool accounting
    let pool = &mut ctx.accounts.stake_pool;
    pool.reward_per_token_stored = updated_reward_per_token;
    pool.last_update_time = clock.unix_timestamp;
    pool.reward_vault_balance = pool
        .reward_vault_balance
        .checked_sub(actual_claim)
        .ok_or(NozzError::MathOverflow)?;
    pool.total_rewards_distributed = pool
        .total_rewards_distributed
        .checked_add(actual_claim)
        .ok_or(NozzError::MathOverflow)?;

    // Reset position's earned counter
    ctx.accounts.stake_position.rewards_earned = 0;
    ctx.accounts.stake_position.reward_per_token_paid = updated_reward_per_token;

    emit!(RewardsClaimed {
        mint: mint_key,
        staker: staker_key,
        amount: actual_claim,
        timestamp: clock.unix_timestamp,
    });

    msg!("Claimed {} reward tokens", actual_claim);

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimStakeRewards<'info> {
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

    /// Reward vault — source of reward tokens, signed by stake_pool PDA
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = stake_pool,
        associated_token::token_program = token_program,
    )]
    pub stake_reward_vault: InterfaceAccount<'info, TokenAccount>,

    /// Staker receives rewards here
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
    )]
    pub stake_position: Account<'info, StakePosition>,

    pub token_program: Program<'info, Token2022>,
}
