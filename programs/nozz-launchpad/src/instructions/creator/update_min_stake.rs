use anchor_lang::prelude::*;

use crate::{error::NozzError, events::MinStakeUpdated, state::StakePool};

pub fn update_min_stake(ctx: Context<UpdateMinStake>, new_min_stake_amount: u64) -> Result<()> {
    require!(new_min_stake_amount > 0, NozzError::ZeroAmount);

    let pool = &mut ctx.accounts.stake_pool;
    let old_min_stake = pool.min_stake_amount;
    let clock = Clock::get()?;

    pool.min_stake_amount = new_min_stake_amount;

    emit!(MinStakeUpdated {
        mint: pool.mint,
        creator: ctx.accounts.creator.key(),
        old_min_stake,
        new_min_stake: new_min_stake_amount,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Min stake updated: {} -> {}",
        old_min_stake,
        new_min_stake_amount
    );

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateMinStake<'info> {
    /// Only the creator of the token can update the subscription threshold
    #[account(
        constraint = creator.key() == stake_pool.creator @ NozzError::UnAuthorized
    )]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [StakePool::SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,
}
