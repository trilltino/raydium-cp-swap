use super::swap_base_input::{hook_account_ranges, Swap};
use crate::curve::calculator::CurveCalculator;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::token::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;

pub fn swap_base_output<'info>(
    ctx: Context<'info, Swap<'info>>,
    max_amount_in: u64,
    amount_out_received: u64,
) -> Result<()> {
    swap_base_output_inner(ctx, max_amount_in, amount_out_received, &[], &[])
}

/// Hook-aware exact-output swap: the same swap with the Transfer Hook accounts of each transfer
/// framed explicitly. Remaining accounts are the input transfer's slice followed by the output
/// transfer's slice; each count is the whole slice (extras, hook program, validation list).
pub fn swap_base_output_v2<'info>(
    ctx: Context<'info, Swap<'info>>,
    max_amount_in: u64,
    amount_out_received: u64,
    input_hook_account_count: u16,
    output_hook_account_count: u16,
) -> Result<()> {
    let (input_range, output_range) = hook_account_ranges(
        ctx.remaining_accounts.len(),
        input_hook_account_count,
        output_hook_account_count,
    )?;
    let input_hook_accounts = ctx.remaining_accounts[input_range].to_vec();
    let output_hook_accounts = ctx.remaining_accounts[output_range].to_vec();
    swap_base_output_inner(
        ctx,
        max_amount_in,
        amount_out_received,
        &input_hook_accounts,
        &output_hook_accounts,
    )
}

fn swap_base_output_inner<'info>(
    ctx: Context<'info, Swap<'info>>,
    max_amount_in: u64,
    amount_out_received: u64,
    input_hook_accounts: &[AccountInfo<'info>],
    output_hook_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    require_gt!(amount_out_received, 0);
    let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
        || block_timestamp < pool_state.open_time
    {
        return err!(ErrorCode::NotApproved);
    }
    let out_transfer_fee = get_transfer_inverse_fee(
        &ctx.accounts.output_token_mint.to_account_info(),
        amount_out_received,
    )?;
    let amount_out_with_transfer_fee = amount_out_received.checked_add(out_transfer_fee).unwrap();

    let SwapParams {
        trade_direction,
        total_input_token_amount,
        total_output_token_amount,
        token_0_price_x64,
        token_1_price_x64,
        is_creator_fee_on_input,
    } = pool_state.get_swap_params(
        ctx.accounts.input_vault.key(),
        ctx.accounts.output_vault.key(),
        ctx.accounts.input_vault.amount,
        ctx.accounts.output_vault.amount,
    )?;
    let constant_before = u128::from(total_input_token_amount)
        .checked_mul(u128::from(total_output_token_amount))
        .unwrap();

    let creator_fee_rate =
        pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);
    let result = CurveCalculator::swap_base_output(
        u128::from(amount_out_with_transfer_fee),
        u128::from(total_input_token_amount),
        u128::from(total_output_token_amount),
        ctx.accounts.amm_config.trade_fee_rate,
        creator_fee_rate,
        ctx.accounts.amm_config.protocol_fee_rate,
        ctx.accounts.amm_config.fund_fee_rate,
        is_creator_fee_on_input,
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;

    let constant_after = u128::from(result.new_input_vault_amount)
        .checked_mul(u128::from(result.new_output_vault_amount))
        .unwrap();

    #[cfg(feature = "enable-log")]
    msg!(
        "input_amount:{}, output_amount:{}, trade_fee:{}, output_transfer_fee:{}, constant_before:{}, constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
        result.input_amount,
        result.output_amount,
        result.trade_fee,
        out_transfer_fee,
        constant_before,
        constant_after,
        is_creator_fee_on_input,
        result.creator_fee,
    );

    // Re-calculate the source amount swapped based on what the curve says
    let (input_transfer_amount, input_transfer_fee) = {
        let input_amount = u64::try_from(result.input_amount).unwrap();
        require_gt!(input_amount, 0);
        let transfer_fee = get_transfer_inverse_fee(
            &ctx.accounts.input_token_mint.to_account_info(),
            input_amount,
        )?;
        let input_transfer_amount = input_amount.checked_add(transfer_fee).unwrap();
        require_gte!(
            max_amount_in,
            input_transfer_amount,
            ErrorCode::ExceededSlippage
        );
        (input_transfer_amount, transfer_fee)
    };
    require_eq!(
        u64::try_from(result.output_amount).unwrap(),
        amount_out_with_transfer_fee
    );
    let (output_transfer_amount, output_transfer_fee) =
        (amount_out_with_transfer_fee, out_transfer_fee);

    pool_state.update_fees(
        u64::try_from(result.protocol_fee).unwrap(),
        u64::try_from(result.fund_fee).unwrap(),
        u64::try_from(result.creator_fee).unwrap(),
        trade_direction,
    )?;

    emit!(SwapEvent {
        pool_id,
        input_vault_before: total_input_token_amount,
        output_vault_before: total_output_token_amount,
        input_amount: u64::try_from(result.input_amount).unwrap(),
        output_amount: u64::try_from(result.output_amount).unwrap(),
        input_transfer_fee,
        output_transfer_fee,
        base_input: false,
        input_mint: ctx.accounts.input_token_mint.key(),
        output_mint: ctx.accounts.output_token_mint.key(),
        trade_fee: u64::try_from(result.trade_fee).unwrap(),
        creator_fee: u64::try_from(result.creator_fee).unwrap(),
        creator_fee_on_input: is_creator_fee_on_input,
    });
    require_gte!(constant_after, constant_before);

    transfer_from_user_to_pool_vault_with_hook_accounts(
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.input_token_account.to_account_info(),
        ctx.accounts.input_vault.to_account_info(),
        ctx.accounts.input_token_mint.to_account_info(),
        ctx.accounts.input_token_program.to_account_info(),
        input_transfer_amount,
        ctx.accounts.input_token_mint.decimals,
        input_hook_accounts,
    )?;

    transfer_from_pool_vault_to_user_with_hook_accounts(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.output_vault.to_account_info(),
        ctx.accounts.output_token_account.to_account_info(),
        ctx.accounts.output_token_mint.to_account_info(),
        ctx.accounts.output_token_program.to_account_info(),
        output_transfer_amount,
        ctx.accounts.output_token_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
        output_hook_accounts,
    )?;

    // update the previous price to the observation
    ctx.accounts.observation_state.load_mut()?.update(
        oracle::block_timestamp(),
        token_0_price_x64,
        token_1_price_x64,
    )?;
    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
