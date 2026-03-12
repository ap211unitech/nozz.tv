use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken, token_2022::Token2022, token_interface::TokenAccount,
};

use crate::{error::NozzError, BondingCurve};

pub fn graduate_to_dex(_ctx: Context<GraduateToDex>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct GraduateToDex<'info> {
    /// Permissionless — anyone can trigger graduation once complete
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [BondingCurve::SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = bonding_curve.complete @ NozzError::BondingCurveNotComplete,
        constraint = !bonding_curve.migrated @ NozzError::AlreadyGraduated,
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    #[account(
        mut,
        seeds = [BondingCurve::VAULT_SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.vault_bump
    )]
    /// CHECK: PDA used as SOL vault
    pub bonding_curve_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = bonding_curve.mint,
        associated_token::authority = bonding_curve,
        associated_token::token_program = token_program
    )]
    pub bonding_curve_token_vault: InterfaceAccount<'info, TokenAccount>,

    /*
    more accounts need to be included here
    */
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
