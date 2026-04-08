use anchor_lang::prelude::*;

#[error_code]
pub enum NozzError {
    #[msg("You are not authorized to perform this action")]
    UnAuthorized,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Supply percentages must sum to exactly 100")]
    InvalidSupplyAllocation,

    #[msg("Invalid fee basis points (max 1000 = 10%)")]
    InvalidFee,

    #[msg("Invalid treasury account")]
    InavlidTreasury,

    #[msg("Invalid token mint")]
    InvalidTokenMint,

    #[msg("Bonding curve is already complete — trading closed, call graduate_to_dex")]
    BondingCurveComplete,

    #[msg("Bonding curve is not complete yet")]
    BondingCurveNotComplete,

    #[msg("Token has already been migrated to DEX")]
    AlreadyGraduated,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Insufficient SOL in bonding curve reserves")]
    InsufficientReserves,

    #[msg("Insufficient tokens in bonding curve")]
    InsufficientTokens,

    #[msg("Token supply exceeded")]
    SupplyExceeded,

    #[msg("Nothing staked")]
    NothingStaked,

    #[msg("Insufficient staked balance")]
    InsufficientStakedBalance,

    #[msg("No rewards to claim")]
    NoRewardsToClaim,

    #[msg("Reward pool is empty")]
    RewardPoolEmpty,
}
