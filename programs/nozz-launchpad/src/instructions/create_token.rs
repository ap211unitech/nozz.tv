use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, MintTo},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    utils::VIRTUAL_SOL_SEED, BondingCurve, BondingCurveVaultSOL, NozzLaunchpadConfig, TokenCreated,
    CREATOR_TOKEN_MINT_DECIMALS,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTokenParams {
    token_name: String,
    token_ticker: String,
    token_description: String,
    token_uri: String,
}

pub fn handler(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
    let config = &ctx.accounts.nozz_launchpad_config;
    let token_mint = ctx.accounts.mint.key();
    let clock = Clock::get()?;

    let total_supply = config.initial_token_supply;
    let bonding_curve_pct = config.bonding_curve_supply_pct as u64;

    // Virtual reserves seed the curve — (bonding_curve_pct)% allocation expressed in raw units
    // with decimals applied, used in x*y=k math
    let bonding_allocation = total_supply
        .checked_mul(bonding_curve_pct)
        .unwrap()
        .checked_div(100)
        .unwrap();

    ctx.accounts.bonding_curve.set_inner(BondingCurve {
        // Initialize bonding curve state
        mint: token_mint,
        creator: ctx.accounts.creator.key(),
        name: params.token_name.clone(),
        symbol: params.token_ticker.clone(),
        uri: params.token_uri.clone(),

        // Virtual SOL seed bootstraps price — no real SOL deposited yet
        virtual_sol_reserves: VIRTUAL_SOL_SEED,
        virtual_token_reserves: bonding_allocation,
        real_sol_reserves: 0,
        real_token_reserves: bonding_allocation,
        total_supply,
        bonding_curve_allocation: bonding_allocation,

        // Snapshot threshold from config so it doesn't change mid-curve
        graduation_sol_threshold: config.graduation_sol_threshold,
        complete: false,
        migrated: false,
        pending_creator_fees: 0,
        created_at: clock.unix_timestamp,
        total_volume: 0,
        bump: ctx.bumps.bonding_curve,
        vault_bump: ctx.bumps.bonding_curve_vault,
    });

    // Mint entire supply to bonding curve token account
    // 40% will be sold via the curve; 60% stays locked for DEX liquidity
    let bonding_curve_seeds: &[&[u8]] = &[
        BondingCurve::SEED,
        token_mint.as_ref(),
        &[ctx.bumps.bonding_curve],
    ];
    let signer_seeds = &[bonding_curve_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.bonding_curve_ata.to_account_info(),
                authority: ctx.accounts.bonding_curve.to_account_info(),
            },
            signer_seeds,
        ),
        total_supply,
    )?;

    emit!(TokenCreated {
        mint: token_mint,
        creator: ctx.accounts.creator.key(),
        name: params.token_name,
        symbol: params.token_ticker,
        uri: params.token_uri,
        bonding_curve: ctx.accounts.bonding_curve.key(),
        total_supply,
        bonding_curve_allocation: bonding_allocation,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        seeds = [NozzLaunchpadConfig::SEED],
        bump
    )]
    pub nozz_launchpad_config: Account<'info, NozzLaunchpadConfig>,

    #[account(
        init,
        payer = creator,
        mint::decimals = CREATOR_TOKEN_MINT_DECIMALS,
        mint::authority = bonding_curve,
        mint::freeze_authority = bonding_curve,
        mint::token_program = token_program
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = creator,
        seeds = [BondingCurve::SEED, mint.key().as_ref()],
        space = BondingCurve::LEN,
        bump
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    /// Pure SOL vault PDA — holds lamports during bonding curve phase
    #[account(
        init,
        payer = creator,
        space = 0,
        seeds = [BondingCurve::VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA used as a pure SOL vault, no data needed
    pub bonding_curve_vault: Account<'info, BondingCurveVaultSOL>,

    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::token_program = token_program
    )]
    pub bonding_curve_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
