use anchor_lang::prelude::*;

pub fn buy_token(ctx: Context<BuyToken>, amount: u64) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    pub system_program: Program<'info, System>,
}
