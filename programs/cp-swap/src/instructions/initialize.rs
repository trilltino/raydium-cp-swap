use crate::curve::CurveCalculator;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::*;
use anchor_lang::{
    accounts::interface_account::InterfaceAccount,
    prelude::*,
    solana_program::{clock, program::invoke, system_instruction},
    system_program,
};
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::spl_token,
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use std::ops::Deref;

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Address paying to create the pool. Can be anyone
    #[account(mut)]
    pub creator: Signer<'info>,

    /// Which config the pool belongs to.
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// CHECK:
    /// pool vault and lp mint authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Initialize an account to store the pool state
    /// PDA account:
    /// seeds = [
    ///     POOL_SEED.as_bytes(),
    ///     amm_config.key().as_ref(),
    ///     token_0_mint.key().as_ref(),
    ///     token_1_mint.key().as_ref(),
    /// ],
    ///
    /// Or random account: must be signed by cli
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    /// Token_0 mint, the key must smaller than token_1 mint.
    #[account(
        constraint = token_0_mint.key() < token_1_mint.key(),
        mint::token_program = token_0_program,
    )]
    pub token_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Token_1 mint, the key must grater then token_0 mint.
    #[account(
        mint::token_program = token_1_program,
    )]
    pub token_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// pool lp mint
    #[account(
        init,
        seeds = [
            POOL_LP_MINT_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        mint::decimals = 9,
        mint::authority = authority,
        payer = creator,
        mint::token_program = token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    /// payer token0 account
    #[account(
        mut,
        token::mint = token_0_mint,
        token::authority = creator,
    )]
    pub creator_token_0: Box<InterfaceAccount<'info, TokenAccount>>,

    /// creator token1 account
    #[account(
        mut,
        token::mint = token_1_mint,
        token::authority = creator,
    )]
    pub creator_token_1: Box<InterfaceAccount<'info, TokenAccount>>,

    /// creator lp token account
    #[account(
        init,
        associated_token::mint = lp_mint,
        associated_token::authority = creator,
        payer = creator,
        token::token_program = token_program,
    )]
    pub creator_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Token_0 vault for the pool, created by contract
    #[account(
        mut,
        seeds = [
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_0_mint.key().as_ref()
        ],
        bump,
    )]
    pub token_0_vault: UncheckedAccount<'info>,

    /// CHECK: Token_1 vault for the pool, created by contract
    #[account(
        mut,
        seeds = [
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_1_mint.key().as_ref()
        ],
        bump,
    )]
    pub token_1_vault: UncheckedAccount<'info>,

    /// create pool fee account
    #[account(
        mut,
        address= crate::create_pool_fee_reveiver::ID,
    )]
    pub create_pool_fee: Box<InterfaceAccount<'info, TokenAccount>>,

    /// an account to store oracle observations
    #[account(
        init,
        seeds = [
            OBSERVATION_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = creator,
        space = ObservationState::LEN
    )]
    pub observation_state: AccountLoader<'info, ObservationState>,

    /// Program to create mint account and mint tokens
    pub token_program: Program<'info, Token>,
    /// Spl token program or token program 2022
    pub token_0_program: Interface<'info, TokenInterface>,
    /// Spl token program or token program 2022
    pub token_1_program: Interface<'info, TokenInterface>,
    /// Program to create an ATA for receiving position NFT
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// To create a new program account
    pub system_program: Program<'info, System>,
    /// Sysvar for program account
    pub rent: Sysvar<'info, Rent>,
    // remaining account
    // #[account(
    //     seeds = [
    //     SUPPORT_MINT_SEED.as_bytes(),
    //     token_0_mint.key().as_ref(),
    // ],
    //     bump
    // )]
    // pub support_mint0_associated: Account<'info, SupportMintAssociated>,

    // #[account(
    //     seeds = [
    //     SUPPORT_MINT_SEED.as_bytes(),
    //     token_1_mint.key().as_ref(),
    // ],
    //     bump
    // )]
    // pub support_mint1_associated: Account<'info, SupportMintAssociated>,
}

pub fn initialize<'info>(
    ctx: Context<'info, Initialize<'info>>,
    init_amount_0: u64,
    init_amount_1: u64,
    open_time: u64,
) -> Result<()> {
    let support_accounts = ctx.remaining_accounts;
    initialize_inner(
        ctx,
        init_amount_0,
        init_amount_1,
        open_time,
        &[],
        &[],
        support_accounts,
    )
}

/// Hook-aware `initialize`: the same operation with the Transfer Hook accounts of each token's
/// transfer framed explicitly. Remaining accounts are the token_0 transfer's slice followed by
/// the token_1 transfer's slice; each count is the whole slice (extras, hook program, validation
/// list), and 0 means no hook for that token.
pub fn initialize_v2<'info>(
    ctx: Context<'info, Initialize<'info>>,
    init_amount_0: u64,
    init_amount_1: u64,
    open_time: u64,
    token_0_hook_account_count: u16,
    token_1_hook_account_count: u16,
) -> Result<()> {
    let (token_0_range, token_1_range, support_range) =
        crate::instructions::swap_base_input::hook_account_ranges_with_tail(
            ctx.remaining_accounts.len(),
            token_0_hook_account_count,
            token_1_hook_account_count,
        )?;
    let token_0_hook_accounts = ctx.remaining_accounts[token_0_range].to_vec();
    let token_1_hook_accounts = ctx.remaining_accounts[token_1_range].to_vec();
    let support_accounts = ctx.remaining_accounts[support_range].to_vec();
    initialize_inner(
        ctx,
        init_amount_0,
        init_amount_1,
        open_time,
        &token_0_hook_accounts,
        &token_1_hook_accounts,
        &support_accounts,
    )
}

fn initialize_inner<'info>(
    ctx: Context<'info, Initialize<'info>>,
    init_amount_0: u64,
    init_amount_1: u64,
    mut open_time: u64,
    token_0_hook_accounts: &[AccountInfo<'info>],
    token_1_hook_accounts: &[AccountInfo<'info>],
    support_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let mint0_associated_is_initialized =
        support_mint_associated_is_initialized(support_accounts, &ctx.accounts.token_0_mint)?;
    let mint1_associated_is_initialized =
        support_mint_associated_is_initialized(support_accounts, &ctx.accounts.token_1_mint)?;
    if !(is_supported_mint(&ctx.accounts.token_0_mint, mint0_associated_is_initialized).unwrap()
        && is_supported_mint(&ctx.accounts.token_1_mint, mint1_associated_is_initialized).unwrap())
    {
        return err!(ErrorCode::NotSupportMint);
    }

    if ctx.accounts.amm_config.disable_create_pool {
        return err!(ErrorCode::NotApproved);
    }
    let block_timestamp = clock::Clock::get()?.unix_timestamp as u64;
    if open_time <= block_timestamp {
        open_time = block_timestamp + 1;
    }
    // due to stack/heap limitations, we have to create redundant new accounts ourselves.
    create_token_account(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.creator.to_account_info(),
        &ctx.accounts.token_0_vault.to_account_info(),
        &ctx.accounts.token_0_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_0_program.to_account_info(),
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_0_mint.key().as_ref(),
            &[ctx.bumps.token_0_vault][..],
        ],
    )?;

    create_token_account(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.creator.to_account_info(),
        &ctx.accounts.token_1_vault.to_account_info(),
        &ctx.accounts.token_1_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_1_program.to_account_info(),
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_1_mint.key().as_ref(),
            &[ctx.bumps.token_1_vault][..],
        ],
    )?;

    let pool_state_loader = create_pool(
        &ctx.accounts.creator.to_account_info(),
        &ctx.accounts.pool_state.to_account_info(),
        &ctx.accounts.amm_config.to_account_info(),
        &ctx.accounts.token_0_mint.to_account_info(),
        &ctx.accounts.token_1_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let pool_state = &mut pool_state_loader.load_init()?;

    let mut observation_state = ctx.accounts.observation_state.load_init()?;
    observation_state.pool_id = ctx.accounts.pool_state.key();

    transfer_from_user_to_pool_vault_with_hook_accounts(
        ctx.accounts.creator.to_account_info(),
        ctx.accounts.creator_token_0.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.token_0_mint.to_account_info(),
        ctx.accounts.token_0_program.to_account_info(),
        init_amount_0,
        ctx.accounts.token_0_mint.decimals,
        token_0_hook_accounts,
    )?;

    transfer_from_user_to_pool_vault_with_hook_accounts(
        ctx.accounts.creator.to_account_info(),
        ctx.accounts.creator_token_1.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.token_1_mint.to_account_info(),
        ctx.accounts.token_1_program.to_account_info(),
        init_amount_1,
        ctx.accounts.token_1_mint.decimals,
        token_1_hook_accounts,
    )?;

    let token_0_vault =
        spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
            ctx.accounts
                .token_0_vault
                .to_account_info()
                .try_borrow_data()?
                .deref(),
        )?
        .base;
    let token_1_vault =
        spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
            ctx.accounts
                .token_1_vault
                .to_account_info()
                .try_borrow_data()?
                .deref(),
        )?
        .base;

    CurveCalculator::validate_supply(token_0_vault.amount, token_1_vault.amount)?;

    let liquidity = U128::from(token_0_vault.amount)
        .checked_mul(token_1_vault.amount.into())
        .unwrap()
        .integer_sqrt()
        .as_u64();
    let lock_lp_amount = 100;
    msg!(
        "liquidity:{}, lock_lp_amount:{}, vault_0_amount:{},vault_1_amount:{}",
        liquidity,
        lock_lp_amount,
        token_0_vault.amount,
        token_1_vault.amount
    );
    token::token_mint_to(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.creator_lp_token.to_account_info(),
        liquidity
            .checked_sub(lock_lp_amount)
            .ok_or(ErrorCode::InitLpAmountTooLess)?,
        &[&[crate::AUTH_SEED.as_bytes(), &[ctx.bumps.authority]]],
    )?;

    // Charge the fee to create a pool
    if ctx.accounts.amm_config.create_pool_fee != 0 {
        invoke(
            &system_instruction::transfer(
                ctx.accounts.creator.key,
                &ctx.accounts.create_pool_fee.key(),
                u64::from(ctx.accounts.amm_config.create_pool_fee),
            ),
            &[
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.create_pool_fee.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        invoke(
            &spl_token::instruction::sync_native(
                ctx.accounts.token_program.key,
                &ctx.accounts.create_pool_fee.key(),
            )?,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.create_pool_fee.to_account_info(),
            ],
        )?;
    }

    pool_state.initialize(
        ctx.bumps.authority,
        liquidity,
        open_time,
        ctx.accounts.creator.key(),
        ctx.accounts.amm_config.key(),
        ctx.accounts.token_0_vault.key(),
        ctx.accounts.token_1_vault.key(),
        &ctx.accounts.token_0_mint,
        &ctx.accounts.token_1_mint,
        ctx.accounts.lp_mint.key(),
        ctx.accounts.lp_mint.decimals,
        ctx.accounts.observation_state.key(),
        CreatorFeeOn::BothToken,
        false,
    );

    Ok(())
}

pub fn create_pool<'info>(
    payer: &AccountInfo<'info>,
    pool_account_info: &AccountInfo<'info>,
    amm_config: &AccountInfo<'info>,
    token_0_mint: &AccountInfo<'info>,
    token_1_mint: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<AccountLoad<'info, PoolState>> {
    if pool_account_info.owner != &system_program::ID {
        return err!(ErrorCode::NotApproved);
    }

    let (expect_pda_address, bump) = Pubkey::find_program_address(
        &[
            POOL_SEED.as_bytes(),
            amm_config.key().as_ref(),
            token_0_mint.key().as_ref(),
            token_1_mint.key().as_ref(),
        ],
        &crate::id(),
    );

    if pool_account_info.key() != expect_pda_address {
        require_eq!(pool_account_info.is_signer, true);
    }

    token::create_or_allocate_account(
        &crate::id(),
        payer.to_account_info(),
        system_program.to_account_info(),
        pool_account_info.clone(),
        &[
            POOL_SEED.as_bytes(),
            amm_config.key().as_ref(),
            token_0_mint.key().as_ref(),
            token_1_mint.key().as_ref(),
            &[bump],
        ],
        PoolState::LEN,
    )?;

    Ok(AccountLoad::<PoolState>::try_from_unchecked(
        &crate::id(),
        &pool_account_info,
    )?)
}
