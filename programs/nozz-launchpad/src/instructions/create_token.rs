use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTokenParams {}

pub fn handler(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct CreateToken {}
