use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        self, token_metadata_initialize, Mint, MintTo, TokenAccount, TokenInterface,
        TokenMetadataInitialize,
    },
};

use crate::{
    error::NozzError, utils::VIRTUAL_SOL_SEED, BondingCurve, BondingCurveVaultSOL,
    NozzLaunchpadConfig, TokenCreated, CREATOR_TOKEN_MINT_DECIMALS,
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

    let total_supply = config.initial_token_supply; // Raw Units
    let bonding_curve_pct = config.bonding_curve_supply_pct as u64;

    // Virtual reserves seed the curve — (bonding_curve_pct)% allocation expressed in raw units
    // with decimals applied, used in x*y=k math
    let bonding_allocation = total_supply
        .checked_mul(bonding_curve_pct)
        .ok_or(NozzError::MathOverflow)?
        .checked_div(100)
        .ok_or(NozzError::MathOverflow)?;

    // PDA signer seeds
    let bonding_curve_seeds: &[&[u8]] = &[
        BondingCurve::SEED,
        token_mint.as_ref(),
        &[ctx.bumps.bonding_curve],
    ];
    let signer_seeds = &[bonding_curve_seeds];

    token_metadata_initialize(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TokenMetadataInitialize {
                program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                metadata: ctx.accounts.mint.to_account_info(), // metadata = mint itself
                mint_authority: ctx.accounts.bonding_curve.to_account_info(),
                update_authority: ctx.accounts.bonding_curve.to_account_info(),
            },
            signer_seeds,
        ),
        params.token_name.clone(),
        params.token_ticker.clone(),
        params.token_uri.clone(),
    )?;

    // Mint entire supply to bonding curve token account
    // 40% will be sold via the curve; 60% stays locked for DEX liquidity
    token_interface::mint_to(
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

    ctx.accounts.bonding_curve.set_inner(BondingCurve {
        // Initialize bonding curve state
        mint: token_mint,
        creator: ctx.accounts.creator.key(),

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

    /// Token-2022 mint with MetadataPointer extension.
    /// Anchor allocates the correct account size for the extension
    /// and validates it as a proper mint via InterfaceAccount<Mint>.
    /// The metadata pointer is set to the mint itself (self-referential),
    /// so name/symbol/uri live inside the mint account — no separate
    /// metadata account needed.
    #[account(
        init,
        payer = creator,
        mint::decimals = CREATOR_TOKEN_MINT_DECIMALS,
        mint::authority = bonding_curve,
        mint::freeze_authority = bonding_curve,
        mint::token_program = token_program,
        extensions::metadata_pointer::authority = bonding_curve,
        extensions::metadata_pointer::metadata_address = mint
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Bonding curve state PDA
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
