pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use events::*;
#[allow(ambiguous_glob_reexports)]
pub use instructions::*;
pub use state::*;

declare_id!("5pAxXXdL7NzFKqpp6TnuxBojeFuKEijX6amRvY4G8dvA");

#[program]
pub mod nozz_launchpad {
    use super::*;

    /// Initialize the global platform config (admin only)
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        initialize_config::initialize_config(ctx, params)
    }

    /// Create a new streamer token with bonding curve
    pub fn create_token(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
        create_token::create_token(ctx, params)
    }
}
