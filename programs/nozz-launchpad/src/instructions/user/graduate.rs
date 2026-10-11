use anchor_lang::{prelude::*, solana_program::program_pack::Pack};
use anchor_spl::{
    associated_token::{self, AssociatedToken},
    token::{self, Token},
    token_2022::{self, Token2022},
    token_interface::{Mint, TokenAccount},
};

use crate::{error::NozzError, BondingCurve, TokenGraduated};

pub fn graduate_to_dex(ctx: Context<GraduateToDex>) -> Result<()> {
    // Graduation converts the remaining curve liquidity into a real Raydium pool.
    // The logic must validate all pool PDAs and custody accounts before moving any funds
    // so the final migration is deterministic, permissionless, and compatible with
    // Raydium's expected canonical pool layout.
    let bonding_curve = &ctx.accounts.bonding_curve;
    let mint = ctx.accounts.mint.key();
    let wsol_mint = ctx.accounts.wsol_mint.key();

    // Raydium requires pool mints in ascending address order.
    let (token_0_mint, token_1_mint) = if mint < wsol_mint {
        (mint, wsol_mint)
    } else {
        (wsol_mint, mint)
    };
    let cp_swap_program = ctx.accounts.raydium_cp_swap_program.key();
    let amm_config = ctx.accounts.amm_config.key();

    let (expected_authority, _) =
        Pubkey::find_program_address(&[raydium_cp_swap::AUTH_SEED.as_bytes()], &cp_swap_program);
    let (expected_pool, _) = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_SEED.as_bytes(),
            amm_config.as_ref(),
            token_0_mint.as_ref(),
            token_1_mint.as_ref(),
        ],
        &cp_swap_program,
    );
    let (expected_lp_mint, _) = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_LP_MINT_SEED.as_bytes(),
            expected_pool.as_ref(),
        ],
        &cp_swap_program,
    );
    let (expected_token_0_vault, _) = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_VAULT_SEED.as_bytes(),
            expected_pool.as_ref(),
            token_0_mint.as_ref(),
        ],
        &cp_swap_program,
    );
    let (expected_token_1_vault, _) = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_VAULT_SEED.as_bytes(),
            expected_pool.as_ref(),
            token_1_mint.as_ref(),
        ],
        &cp_swap_program,
    );
    let (expected_observation, _) = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::OBSERVATION_SEED.as_bytes(),
            expected_pool.as_ref(),
        ],
        &cp_swap_program,
    );

    require_keys_eq!(
        ctx.accounts.raydium_authority.key(),
        expected_authority,
        NozzError::InvalidDexAccount
    );
    require_keys_eq!(
        ctx.accounts.raydium_pool_state.key(),
        expected_pool,
        NozzError::InvalidDexAccount
    );
    require_keys_eq!(
        ctx.accounts.raydium_lp_mint.key(),
        expected_lp_mint,
        NozzError::InvalidDexAccount
    );
    require_keys_eq!(
        ctx.accounts.token_0_vault.key(),
        expected_token_0_vault,
        NozzError::InvalidDexAccount
    );
    require_keys_eq!(
        ctx.accounts.token_1_vault.key(),
        expected_token_1_vault,
        NozzError::InvalidDexAccount
    );
    require_keys_eq!(
        ctx.accounts.observation_state.key(),
        expected_observation,
        NozzError::InvalidDexAccount
    );

    let expected_caller_lp_token =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &ctx.accounts.caller.key(),
            &expected_lp_mint,
            &Token::id(),
        );
    require_keys_eq!(
        ctx.accounts.caller_lp_token.key(),
        expected_caller_lp_token,
        NozzError::InvalidDexAccount
    );
    let expected_locked_lp_token =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &bonding_curve.key(),
            &expected_lp_mint,
            &Token::id(),
        );
    require_keys_eq!(
        ctx.accounts.locked_lp_token.key(),
        expected_locked_lp_token,
        NozzError::InvalidDexAccount
    );

    // The pool should receive only the usable SOL balance from the curve vault.
    // We intentionally exclude the rent reserve (the vault's own account storage) and any
    // accrued creator fees so the creator can withdraw fees without poisoning the pool
    // liquidity deposit.
    let vault_info = ctx.accounts.bonding_curve_sol_vault.to_account_info();
    let creator_fee = bonding_curve.claimable_creator_fees;
    let rent_reserve = Rent::get()?.minimum_balance(vault_info.data_len());
    let available_lamports = vault_info
        .lamports()
        .checked_sub(rent_reserve)
        .ok_or(NozzError::InsufficientReserves)?;
    let sol_amount = available_lamports
        .checked_sub(creator_fee)
        .ok_or(NozzError::InsufficientReserves)?;
    let token_amount = ctx.accounts.bonding_curve_token_vault.amount;
    require!(sol_amount > 0, NozzError::InsufficientReserves);
    require!(token_amount > 0, NozzError::InsufficientTokens);

    {
        let mut vault_lamports = vault_info.try_borrow_mut_lamports()?;
        **vault_lamports = vault_lamports
            .checked_sub(available_lamports)
            .ok_or(NozzError::InsufficientReserves)?;
    }
    {
        let creator_info = ctx.accounts.creator.to_account_info();
        let mut creator_lamports = creator_info.try_borrow_mut_lamports()?;
        **creator_lamports = creator_lamports
            .checked_add(creator_fee)
            .ok_or(NozzError::MathOverflow)?;
    }
    {
        let wsol_info = ctx.accounts.caller_wsol_ata.to_account_info();
        let mut wsol_lamports = wsol_info.try_borrow_mut_lamports()?;
        **wsol_lamports = wsol_lamports
            .checked_add(sol_amount)
            .ok_or(NozzError::MathOverflow)?;
    }

    // Sync the WSOL ATA after moving lamports out of the vault so the wrapped SOL token
    // balance matches the underlying lamports. The extra account metas are included to
    // keep the nested CPI aligned with the lamport transfers done just above this block.
    let mut sync_native_ix = anchor_spl::token::spl_token::instruction::sync_native(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.caller_wsol_ata.key(),
    )?;
    sync_native_ix.accounts.extend([
        anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
            vault_info.key(),
            false,
        ),
        anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
            ctx.accounts.creator.key(),
            false,
        ),
    ]);
    anchor_lang::solana_program::program::invoke(
        &sync_native_ix,
        &[
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.caller_wsol_ata.to_account_info(),
            vault_info,
            ctx.accounts.creator.to_account_info(),
        ],
    )?;

    let bonding_curve_seeds: &[&[u8]] = &[
        BondingCurve::SEED,
        bonding_curve.mint.as_ref(),
        &[bonding_curve.bump],
    ];
    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            token_2022::TransferChecked {
                from: ctx.accounts.bonding_curve_token_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.caller_token_ata.to_account_info(),
                authority: bonding_curve.to_account_info(),
            },
            &[bonding_curve_seeds],
        ),
        token_amount,
        ctx.accounts.mint.decimals,
    )?;

    let (source_0, source_1, token_program_0, token_program_1, amount_0, amount_1) =
        if mint < wsol_mint {
            (
                ctx.accounts.caller_token_ata.to_account_info(),
                ctx.accounts.caller_wsol_ata.to_account_info(),
                ctx.accounts.token_2022_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                token_amount,
                sol_amount,
            )
        } else {
            (
                ctx.accounts.caller_wsol_ata.to_account_info(),
                ctx.accounts.caller_token_ata.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.token_2022_program.to_account_info(),
                sol_amount,
                token_amount,
            )
        };

    // Create the Raydium CPMM pool and deposit the curve's remaining token + WSOL
    // liquidity in a single CPI. The caller receives LP tokens as a service fee for
    // unlocking the migration, while the bonding curve keeps custody of the locked LP
    // position to preserve the on-chain ownership model.
    raydium_cp_swap::cpi::initialize(
        CpiContext::new(
            ctx.accounts.raydium_cp_swap_program.to_account_info(),
            raydium_cp_swap::cpi::accounts::Initialize {
                creator: ctx.accounts.caller.to_account_info(),
                amm_config: ctx.accounts.amm_config.to_account_info(),
                authority: ctx.accounts.raydium_authority.to_account_info(),
                pool_state: ctx.accounts.raydium_pool_state.to_account_info(),
                token_0_mint: if mint < wsol_mint {
                    ctx.accounts.mint.to_account_info()
                } else {
                    ctx.accounts.wsol_mint.to_account_info()
                },
                token_1_mint: if mint < wsol_mint {
                    ctx.accounts.wsol_mint.to_account_info()
                } else {
                    ctx.accounts.mint.to_account_info()
                },
                lp_mint: ctx.accounts.raydium_lp_mint.to_account_info(),
                creator_token_0: source_0,
                creator_token_1: source_1,
                creator_lp_token: ctx.accounts.caller_lp_token.to_account_info(),
                token_0_vault: ctx.accounts.token_0_vault.to_account_info(),
                token_1_vault: ctx.accounts.token_1_vault.to_account_info(),
                create_pool_fee: ctx.accounts.create_pool_fee.to_account_info(),
                observation_state: ctx.accounts.observation_state.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                token_0_program: token_program_0,
                token_1_program: token_program_1,
                associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ),
        amount_0,
        amount_1,
        0,
    )?;

    associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        associated_token::Create {
            payer: ctx.accounts.caller.to_account_info(),
            associated_token: ctx.accounts.locked_lp_token.to_account_info(),
            authority: bonding_curve.to_account_info(),
            mint: ctx.accounts.raydium_lp_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        },
    ))?;

    let lp_token_amount = {
        let lp_account_data = ctx.accounts.caller_lp_token.try_borrow_data()?;
        let lp_account = anchor_spl::token::spl_token::state::Account::unpack(&lp_account_data)?;
        lp_account.amount
    };
    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::TransferChecked {
                from: ctx.accounts.caller_lp_token.to_account_info(),
                mint: ctx.accounts.raydium_lp_mint.to_account_info(),
                to: ctx.accounts.locked_lp_token.to_account_info(),
                authority: ctx.accounts.caller.to_account_info(),
            },
        ),
        lp_token_amount,
        9,
    )?;

    // The LP position is locked to the bonding-curve PDA, not the permissionless caller.
    // This keeps the migrated liquidity controlled by the token's program-owned identity
    // instead of allowing an arbitrary user to drain the pool after graduation.
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.real_sol_reserves = 0;
    bonding_curve.real_token_reserves = 0;
    bonding_curve.claimable_creator_fees = 0;
    bonding_curve.migrated = true;

    emit!(TokenGraduated {
        mint,
        bonding_curve: bonding_curve.key(),
        pool: expected_pool,
        sol_amount,
        token_amount,
        locked_lp_tokens: lp_token_amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct GraduateToDex<'info> {
    /// Permissionless — anyone can trigger graduation once complete.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(mut, address = bonding_curve.creator @ NozzError::InvalidDexAccount)]
    /// CHECK: The stored creator receives any unclaimed creator fees.
    pub creator: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [BondingCurve::SEED, bonding_curve.mint.as_ref()],
        bump = bonding_curve.bump,
        constraint = bonding_curve.complete @ NozzError::BondingCurveNotComplete,
        constraint = !bonding_curve.migrated @ NozzError::AlreadyGraduated,
    )]
    pub bonding_curve: Box<Account<'info, BondingCurve>>,

    #[account(
        address = bonding_curve.mint,
        mint::token_program = token_2022_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = anchor_spl::token::spl_token::native_mint::ID)]
    pub wsol_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [BondingCurve::VAULT_SEED, bonding_curve.mint.as_ref()],
        bump = bonding_curve.vault_bump
    )]
    /// CHECK: Program-owned SOL vault PDA.
    pub bonding_curve_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::token_program = token_2022_program
    )]
    pub bonding_curve_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = mint,
        associated_token::authority = caller,
        associated_token::token_program = token_2022_program
    )]
    pub caller_token_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = wsol_mint,
        associated_token::authority = caller,
        associated_token::token_program = token_program
    )]
    pub caller_wsol_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        address = raydium_cp_swap::create_pool_fee_reveiver::ID
    )]
    pub create_pool_fee: Box<InterfaceAccount<'info, TokenAccount>>,

    pub amm_config: Box<Account<'info, raydium_cp_swap::states::AmmConfig>>,

    /// CHECK: Validated against the canonical Raydium CPMM PDA.
    pub raydium_authority: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub raydium_pool_state: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub raydium_lp_mint: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub caller_lp_token: UncheckedAccount<'info>,
    /// CHECK: Created after pool initialization as the bonding curve's LP-token custody account.
    #[account(mut)]
    pub locked_lp_token: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub token_0_vault: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub token_1_vault: UncheckedAccount<'info>,
    /// CHECK: Created by the Raydium CPMM CPI after validating its canonical PDA.
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub raydium_cp_swap_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,
}
