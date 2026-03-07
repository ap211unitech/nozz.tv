pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;

declare_id!("HPzJyeHvXQ8vTMkosr6DV1T3JBZQr1t9VinBS7FA7614");

#[program]
pub mod nozz_dex {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }
}
