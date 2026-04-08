use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{transfer_checked, Token2022, TransferChecked},
    token_interface::{Mint, TokenAccount},
};

use crate::{
    error::NozzError, utils::get_sol_for_tokens, BondingCurve, NozzLaunchpadConfig, TradeEvent,
    CREATOR_TOKEN_MINT_DECIMALS,
};

pub fn sell_token(ctx: Context<SellToken>, token_amount: u64, min_sol_out: u64) -> Result<()> {
    require!(token_amount > 0, NozzError::ZeroAmount);

    let clock = Clock::get()?;
    let token_mint = &ctx.accounts.mint;
    let bonding_curve = &ctx.accounts.bonding_curve;

    // bonding curve math
    let sol_out = get_sol_for_tokens(
        bonding_curve.virtual_sol_reserves,
        bonding_curve.virtual_token_reserves,
        token_amount,
    )?;

    // slippage control
    require!(sol_out >= min_sol_out, NozzError::SlippageExceeded);
    require!(
        sol_out <= bonding_curve.real_sol_reserves,
        NozzError::InsufficientReserves
    );

    // Transfer Tokens: seller ATA -> bonding_curve ATA
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.seller_token_ata.to_account_info(),
                to: ctx.accounts.bonding_curve_token_vault.to_account_info(),
                mint: token_mint.to_account_info(),
                authority: ctx.accounts.seller.to_account_info(),
            },
        ),
        token_amount,
        token_mint.decimals,
    )?;

    // Transfer SOL: vault -> seller (net)
    // Direct lamport manipulation — vault is a PDA with no data, safe to mutate
    **ctx
        .accounts
        .bonding_curve_sol_vault
        .try_borrow_mut_lamports()? -= sol_out;
    **ctx.accounts.seller.try_borrow_mut_lamports()? += sol_out;

    // Update state
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_sol_reserves = bonding_curve
        .virtual_sol_reserves
        .checked_sub(sol_out)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.virtual_token_reserves = bonding_curve
        .virtual_token_reserves
        .checked_add(token_amount)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.real_sol_reserves = bonding_curve
        .real_sol_reserves
        .checked_sub(sol_out)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.real_token_reserves = bonding_curve
        .real_token_reserves
        .checked_add(token_amount)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.total_volume = bonding_curve
        .total_volume
        .checked_add(sol_out)
        .ok_or(NozzError::MathOverflow)?;

    emit!(TradeEvent {
        mint: bonding_curve.mint,
        trader: ctx.accounts.seller.key(),
        is_buy: false,
        sol_amount: sol_out,
        token_amount,
        platform_fee: 0,
        creator_fee: 0,
        virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SellToken<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        seeds = [NozzLaunchpadConfig::SEED],
        bump = nozz_launchpad_config.bump
    )]
    pub nozz_launchpad_config: Account<'info, NozzLaunchpadConfig>,

    #[account(
        mint::decimals = CREATOR_TOKEN_MINT_DECIMALS,
        mint::authority = bonding_curve,
        mint::freeze_authority = bonding_curve,
        mint::token_program = token_program,
        extensions::metadata_pointer::authority = bonding_curve,
        extensions::metadata_pointer::metadata_address = mint,
        seeds = [BondingCurve::CREATOR_TOKEN_MINT_SEED, bonding_curve.creator.key().as_ref()],
        bump,
        constraint = mint.key() == bonding_curve.mint @ NozzError::InvalidTokenMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [BondingCurve::SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = !bonding_curve.complete @ NozzError::BondingCurveComplete,
        constraint = !bonding_curve.migrated @ NozzError::AlreadyGraduated,
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    #[account(
        mut,
        seeds = [BondingCurve::VAULT_SEED, mint.key().as_ref()],
        bump = bonding_curve.vault_bump
    )]
    /// CHECK: PDA used as SOL vault
    pub bonding_curve_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::token_program = token_program
    )]
    pub bonding_curve_token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = seller,
        associated_token::token_program = token_program
    )]
    pub seller_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
