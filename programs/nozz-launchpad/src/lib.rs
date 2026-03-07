pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("6zp1FgL5FShDjJoh8hoBztncuYWvSANvPHUZFceiVFsy");

#[program]
pub mod nozz_launchpad {
    use super::*;

    /// Initialize the global platform config (admin only)
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        initialize_config::handler(ctx, params)
    }

    /// Create a new streamer token with bonding curve
    pub fn create_token(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
        create_token::handler(ctx, params)
    }
}
