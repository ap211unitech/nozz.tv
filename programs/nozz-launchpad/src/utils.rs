use anchor_lang::prelude::*;

use crate::error::NozzError;

/// Constant product bonding curve: x * y = k
/// This mirrors pump.fun's implementation with virtual reserves.
///
/// bonding_curve.virtual_sol_reserves = 30_000_000_000; // 30 SOL — never in any vault
/// bonding_curve.virtual_token_reserves = 40_000_000; // 100M * 0.4 = 40M tokens (just a number in state)
///
/// No lamports move. No tokens are reserved. It's purely arithmetic scaffolding.
///
/// ---
///
/// **Initial state**
/// ```
/// virtual_sol    = 30 SOL
/// virtual_tokens = 40,000,000
///
/// k = x * y
///   = 30 * 40,000,000
///   = 1,200,000,000
///
/// real_sol       = 0
/// real_tokens    = 40,000,000
/// ```
///
/// ---
///
/// **What actually happens on a buy**
///
/// ```
/// // Buyer sends 1 SOL
///
/// tokens_out = virtual_tokens - k / (virtual_sol + sol_in)
///
/// tokens_out = 40,000,000 - 1,200,000,000 / (30 + 1)
///            = 40,000,000 - 38,709,677.419
///            = 1,290,322.581 tokens
///
/// ≈ 1,290,323 tokens
/// ```
///
/// ---
///
/// **State after trade**
///
/// ```
/// virtual_sol    = 31 SOL
/// virtual_tokens = 38,709,677.419
///
/// real_sol       = 0.99 SOL      // 1% fee taken
/// real_tokens    = 38,709,677.419
/// ```
///
/// ---
///
/// **What virtual SOL actually controls**
///
/// It determines the **starting price and curve slope**:
///
/// ```
/// starting_price = virtual_sol / virtual_tokens
///
///                = 30 / 40,000,000
///                = 0.00000075 SOL per token
/// ```
///
/// Virtual reserves move with every trade to maintain **x * y = k**,  
/// while the vault only ever holds **real SOL from buyers**.

pub const VIRTUAL_SOL_SEED: u64 = 30_000_000_000; // 30 SOL in lamports

/// Calculate fee amount from basis points
pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(NozzError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(NozzError::MathOverflow)?;
    Ok(fee as u64)
}

/// Given SOL in, calculate tokens out (buy direction)
///
/// Formula (constant product):
///   new_sol_reserves = sol_reserves + sol_in
///   new_token_reserves = k / new_sol_reserves
///   tokens_out = token_reserves - new_token_reserves
pub fn get_tokens_for_sol(
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
    sol_in: u64,
) -> Result<u64> {
    require!(sol_in > 0, NozzError::ZeroAmount);

    // k = x * y  (use u128 to prevent overflow)
    let k: u128 = (virtual_sol_reserves as u128)
        .checked_mul(virtual_token_reserves as u128)
        .ok_or(NozzError::MathOverflow)?;

    let new_sol: u128 = (virtual_sol_reserves as u128)
        .checked_add(sol_in as u128)
        .ok_or(NozzError::MathOverflow)?;

    let new_tokens: u128 = k.checked_div(new_sol).ok_or(NozzError::MathOverflow)?;

    let tokens_out = (virtual_token_reserves as u128)
        .checked_sub(new_tokens)
        .ok_or(NozzError::InsufficientTokens)?;

    Ok(tokens_out as u64)
}

/// Given tokens in, calculate SOL out (sell direction)
///
/// Formula (constant product):
///   new_token_reserves = token_reserves + tokens_in
///   new_sol_reserves = k / new_token_reserves
///   sol_out = sol_reserves - new_sol_reserves
pub fn get_sol_for_tokens(
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
    tokens_in: u64,
) -> Result<u64> {
    require!(tokens_in > 0, NozzError::ZeroAmount);

    let k: u128 = (virtual_sol_reserves as u128)
        .checked_mul(virtual_token_reserves as u128)
        .ok_or(NozzError::MathOverflow)?;

    let new_tokens: u128 = (virtual_token_reserves as u128)
        .checked_add(tokens_in as u128)
        .ok_or(NozzError::MathOverflow)?;

    let new_sol: u128 = k.checked_div(new_tokens).ok_or(NozzError::MathOverflow)?;

    let sol_out = (virtual_sol_reserves as u128)
        .checked_sub(new_sol)
        .ok_or(NozzError::InsufficientReserves)?;

    Ok(sol_out as u64)
}

/// Calculate current price in lamports per token
pub fn get_current_price(virtual_sol_reserves: u64, virtual_token_reserves: u64) -> Result<u64> {
    require!(virtual_token_reserves > 0, NozzError::MathOverflow);

    let price = (virtual_sol_reserves as u128)
        .checked_mul(1_000_000) // scale for precision
        .ok_or(NozzError::MathOverflow)?
        .checked_div(virtual_token_reserves as u128)
        .ok_or(NozzError::MathOverflow)?;

    Ok(price as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buy_sell_symmetry() {
        let sol_res: u64 = 30_000_000_000;
        let tok_res: u64 = 793_100_000_000_000;

        // Buy with 1 SOL
        let sol_in = 1_000_000_000u64;
        let tokens_out = get_tokens_for_sol(sol_res, tok_res, sol_in).unwrap();
        println!("Tokens out for 1 SOL: {}", tokens_out);

        // Sell those tokens back (should get approximately 1 SOL minus rounding)
        let new_sol_res = sol_res + sol_in;
        let new_tok_res = tok_res - tokens_out;
        let sol_back = get_sol_for_tokens(new_sol_res, new_tok_res, tokens_out).unwrap();
        println!("SOL back: {}", sol_back);

        // Should be very close to sol_in (within rounding)
        let diff = if sol_in > sol_back {
            sol_in - sol_back
        } else {
            sol_back - sol_in
        };
        assert!(diff < 1000, "Symmetry error too large: {}", diff);
    }

    #[test]
    fn test_price_increases_with_buys() {
        let mut sol_res: u64 = 30_000_000_000;
        let mut tok_res: u64 = 793_100_000_000_000;

        let price_before = get_current_price(sol_res, tok_res).unwrap();

        // Simulate 10 SOL buy
        let sol_in = 10_000_000_000u64;
        let tokens_out = get_tokens_for_sol(sol_res, tok_res, sol_in).unwrap();
        sol_res += sol_in;
        tok_res -= tokens_out;

        let price_after = get_current_price(sol_res, tok_res).unwrap();
        println!("Price before: {}, after: {}", price_before, price_after);
        assert!(
            price_after > price_before,
            "Price should increase after buy"
        );
    }
}
