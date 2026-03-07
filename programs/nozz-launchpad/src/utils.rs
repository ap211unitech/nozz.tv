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
