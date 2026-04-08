use anchor_lang::prelude::*;

use crate::{error::NozzError, BondingCurve, FeesClaimed};

pub fn claim_fees(ctx: Context<ClaimFees>) -> Result<()> {
    let clock = Clock::get()?;
    let creator = &ctx.accounts.creator;
    let bonding_curve = &mut ctx.accounts.bonding_curve;

    let fees_to_claim = bonding_curve.claimable_creator_fees;
    require!(fees_to_claim > 0, NozzError::ZeroAmount);

    // Transfer fees from vault to creator
    **ctx
        .accounts
        .bonding_curve_sol_vault
        .try_borrow_mut_lamports()? -= fees_to_claim;
    **ctx.accounts.creator.try_borrow_mut_lamports()? += fees_to_claim;

    // Reset pending fees
    bonding_curve.claimable_creator_fees = 0;

    emit!(FeesClaimed {
        bonding_curve: bonding_curve.key(),
        mint: bonding_curve.mint.key(),
        creator: creator.key(),
        amount: fees_to_claim,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimFees<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        seeds = [BondingCurve::SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = bonding_curve.creator == creator.key() @ NozzError::UnAuthorized,
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    #[account(
        mut,
        seeds = [BondingCurve::VAULT_SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.vault_bump
    )]
    /// CHECK: PDA used as SOL vault
    pub bonding_curve_sol_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
