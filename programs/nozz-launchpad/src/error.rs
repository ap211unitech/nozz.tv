use anchor_lang::prelude::*;

#[error_code]
pub enum NozzError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Invalid fee basis points (max 1000 = 10%)")]
    InvalidFee,
}
