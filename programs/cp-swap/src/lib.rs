pub mod curve;
pub mod error;
pub mod instructions;
pub mod states;
pub mod utils;
use crate::curve::fees::FEE_RATE_DENOMINATOR_VALUE;
use anchor_lang::prelude::*;
use instructions::*;
pub use states::CreatorFeeOn;

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "raydium-cp-swap",
    project_url: "https://raydium.io",
    contacts: "link:https://immunefi.com/bounty/raydium",
    policy: "https://immunefi.com/bounty/raydium",
    source_code: "https://github.com/raydium-io/raydium-cp-swap",
    preferred_languages: "en",
    auditors: "https://github.com/raydium-io/raydium-docs/blob/master/audit/MadShield%20Q1%202024/raydium-cp-swap-v-1.0.0.pdf"
}

#[cfg(feature = "devnet")]
declare_id!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");
#[cfg(feature = "integration")]
declare_id!("7tRJH4mmEfNGGLf9E8qEvo3oSjq2b75DhSggb1Wz45fJ");
#[cfg(not(any(feature = "devnet", feature = "integration")))]
declare_id!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");

#[cfg(all(feature = "integration", any(feature = "devnet", feature = "localnet")))]
compile_error!("the `integration` feature cannot be combined with `devnet` or `localnet`");

#[cfg(test)]
mod versioned_instruction_abi_tests {
    use anchor_lang::InstructionData;

    #[test]
    fn swap_base_input_v1_layout_is_unchanged() {
        let data = crate::instruction::SwapBaseInput {
            amount_in: 0x0102_0304_0506_0708,
            minimum_amount_out: 0x1112_1314_1516_1718,
        }
        .data();

        assert_eq!(&data[..8], &[143, 190, 90, 218, 196, 30, 51, 222]);
        assert_eq!(
            &data[8..],
            &[8, 7, 6, 5, 4, 3, 2, 1, 24, 23, 22, 21, 20, 19, 18, 17]
        );
    }

    #[test]
    fn swap_base_input_v2_frames_each_transfer_slice_in_data() {
        let data = crate::instruction::SwapBaseInputV2 {
            amount_in: 1,
            minimum_amount_out: 2,
            input_hook_account_count: 3,
            output_hook_account_count: 4,
        }
        .data();

        assert_eq!(&data[..8], &[179, 135, 209, 217, 135, 75, 40, 58]);
        assert_eq!(&data[24..], &[3, 0, 4, 0]);
    }

    #[test]
    fn swap_base_output_v2_frames_each_transfer_slice_in_data() {
        let data = crate::instruction::SwapBaseOutputV2 {
            max_amount_in: 1,
            amount_out: 2,
            input_hook_account_count: 5,
            output_hook_account_count: 6,
        }
        .data();

        // The V1 `swap_base_output` discriminator is unchanged; V2 has its own.
        assert_eq!(
            &crate::instruction::SwapBaseOutput {
                max_amount_in: 1,
                amount_out: 2,
            }
            .data()[..8],
            &[55, 217, 98, 86, 163, 74, 180, 173]
        );
        assert_eq!(&data[..8], &[29, 143, 223, 109, 3, 111, 151, 147]);
        assert_eq!(&data[8..16], &1u64.to_le_bytes());
        assert_eq!(&data[16..24], &2u64.to_le_bytes());
        assert_eq!(&data[24..], &[5, 0, 6, 0]);
    }

    /// Every hook-aware `_v2` instruction has its own discriminator, and the original instruction
    /// keeps the one it always had (so an existing client is unaffected).
    #[test]
    fn two_token_operations_keep_their_v1_discriminators_and_add_v2() {
        use anchor_lang::Discriminator;
        use crate::instruction::*;
        let pinned: [(&[u8], [u8; 8]); 16] = [
            (Deposit::DISCRIMINATOR, [242, 35, 198, 137, 82, 225, 242, 182]),
            (DepositV2::DISCRIMINATOR, [109, 75, 69, 153, 172, 218, 146, 19]),
            (Withdraw::DISCRIMINATOR, [183, 18, 70, 156, 148, 109, 161, 34]),
            (WithdrawV2::DISCRIMINATOR, [242, 80, 163, 0, 196, 221, 194, 194]),
            (CollectProtocolFee::DISCRIMINATOR, [136, 136, 252, 221, 194, 66, 126, 89]),
            (CollectProtocolFeeV2::DISCRIMINATOR, [246, 11, 93, 67, 221, 244, 185, 10]),
            (CollectFundFee::DISCRIMINATOR, [167, 138, 78, 149, 223, 194, 6, 126]),
            (CollectFundFeeV2::DISCRIMINATOR, [21, 250, 142, 236, 215, 232, 49, 184]),
            (CollectCreatorFee::DISCRIMINATOR, [20, 22, 86, 123, 198, 28, 219, 132]),
            (CollectCreatorFeeV2::DISCRIMINATOR, [207, 17, 138, 242, 4, 34, 19, 56]),
            (CollectCreatorFeePermissionless::DISCRIMINATOR, [202, 202, 34, 83, 226, 122, 145, 229]),
            (CollectCreatorFeePermissionlessV2::DISCRIMINATOR, [100, 50, 213, 79, 188, 138, 6, 207]),
            (Initialize::DISCRIMINATOR, [175, 175, 109, 31, 13, 152, 155, 237]),
            (InitializeV2::DISCRIMINATOR, [67, 153, 175, 39, 218, 16, 38, 32]),
            (InitializeWithPermission::DISCRIMINATOR, [63, 55, 254, 65, 49, 178, 89, 121]),
            (InitializeWithPermissionV2::DISCRIMINATOR, [20, 6, 23, 116, 191, 226, 176, 71]),
        ];
        for (found, expected) in pinned {
            assert_eq!(found, expected);
        }
    }

    #[test]
    fn two_token_v2_operations_frame_both_slices_after_their_arguments() {
        let deposit = crate::instruction::DepositV2 {
            lp_token_amount: 1,
            maximum_token_0_amount: 2,
            maximum_token_1_amount: 3,
            token_0_hook_account_count: 4,
            token_1_hook_account_count: 5,
        }
        .data();
        assert_eq!(&deposit[32..], &[4, 0, 5, 0]);

        let fee = crate::instruction::CollectProtocolFeeV2 {
            amount_0_requested: 1,
            amount_1_requested: 2,
            token_0_hook_account_count: 6,
            token_1_hook_account_count: 7,
        }
        .data();
        assert_eq!(&fee[24..], &[6, 0, 7, 0]);

        // No arguments: the two counts follow the discriminator directly.
        let creator = crate::instruction::CollectCreatorFeeV2 {
            token_0_hook_account_count: 2,
            token_1_hook_account_count: 3,
        }
        .data();
        assert_eq!(&creator[8..], &[2, 0, 3, 0]);
    }
}

pub mod admin {
    #[cfg(not(feature = "localnet"))]
    use super::pubkey;
    use super::Pubkey;
    #[cfg(feature = "localnet")]
    pub const ID: Pubkey = Pubkey::from_str_const(env!(
        "CPSWAP_LOCALNET_ADMIN",
        "the `localnet` feature needs CPSWAP_LOCALNET_ADMIN=<admin pubkey> at build time (run `yarn test:local-admin`)"
    ));
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("DRayqG9RXYi8WHgWEmRQGrUWRWbhjYWYkCRJDd6JBBak");
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm");
    #[cfg(all(
        not(feature = "devnet"),
        not(feature = "localnet"),
        not(feature = "integration")
    ))]
    pub const ID: Pubkey = pubkey!("GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ");
}

pub mod create_pool_fee_reveiver {
    use super::{pubkey, Pubkey};
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("3oE58BKVt8KuYkGxx8zBojugnymWmBiyafWgMrnb6eYy");
    /// Must be a wrapped-SOL token account (the program charges the pool-creation fee into it).
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("CnoYEaFeS92rnvfY7i1WHY3yKnXgUQqs1xiC1eVj2aYS");
    #[cfg(not(any(feature = "devnet", feature = "integration")))]
    pub const ID: Pubkey = pubkey!("DNXgeM9EiiaAbaWvwjHj9fQQLAX5ZsfHyvmYUNRAdNC8");
}

pub mod collect_lamports {
    use super::{pubkey, Pubkey};
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("DRaydJNq54dSDHUqYCE3G8YySgaXfZucbh7dTXw9fBMs");
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm");
    #[cfg(not(any(feature = "devnet", feature = "integration")))]
    pub const ID: Pubkey = pubkey!("RayGkhY93thaTgCv98sx1pNLgBHhJDxWUeZXp4bjmnp");
}

pub mod fund_fee_owner {
    use super::{pubkey, Pubkey};
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("DRay33UmULQCeawH3dVpJfN3uqLj6Qtq4ymSRx2pAgGK");
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm");
    #[cfg(not(any(feature = "devnet", feature = "integration")))]
    pub const ID: Pubkey = pubkey!("FUNDduJTA7XcckKHKfAoEnnhuSud2JUCUZv6opWEjrBU");
}

pub mod protocol_fee_owner {
    use super::{pubkey, Pubkey};
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("DRay33UmULQCeawH3dVpJfN3uqLj6Qtq4ymSRx2pAgGK");
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm");
    #[cfg(not(any(feature = "devnet", feature = "integration")))]
    pub const ID: Pubkey = pubkey!("ProCXqRcXJjoUd1RNoo28bSizAA6EEqt9wURZYPDc5u");
}

pub mod shared_creator_fee_owner {
    use super::{pubkey, Pubkey};
    #[cfg(feature = "devnet")]
    pub const ID: Pubkey = pubkey!("DRay2aRSqmGVMkcvsQNU4iskM1ztX31EP7pyrtrntqBQ");
    #[cfg(feature = "integration")]
    pub const ID: Pubkey = pubkey!("QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm");
    #[cfg(not(any(feature = "devnet", feature = "integration")))]
    pub const ID: Pubkey = pubkey!("RayRrPVNAg3hTPa1yfZiR49FqCpkmiLaP4yWsVBnoBZ");
}

pub const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";

#[program]
pub mod raydium_cp_swap {
    use super::*;

    // The configuration of AMM protocol, include trade fee and protocol fee
    /// # Arguments
    ///
    /// * `ctx`- The accounts needed by instruction.
    /// * `index` - The index of amm config, there may be multiple config.
    /// * `trade_fee_rate` - Trade fee rate, can be changed.
    /// * `protocol_fee_rate` - The rate of protocol fee within trade fee.
    /// * `fund_fee_rate` - The rate of fund fee within trade fee.
    ///
    pub fn create_amm_config(
        ctx: Context<CreateAmmConfig>,
        index: u16,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: u64,
        creator_fee_rate: u64,
    ) -> Result<()> {
        assert!(trade_fee_rate + creator_fee_rate < FEE_RATE_DENOMINATOR_VALUE);
        assert!(protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        assert!(fund_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        assert!(fund_fee_rate + protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        instructions::create_amm_config(
            ctx,
            index,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
            create_pool_fee,
            creator_fee_rate,
        )
    }

    /// Updates the owner of the amm config
    /// Must be called by the current owner or admin
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `trade_fee_rate`- The new trade fee rate of amm config, be set when `param` is 0
    /// * `protocol_fee_rate`- The new protocol fee rate of amm config, be set when `param` is 1
    /// * `fund_fee_rate`- The new fund fee rate of amm config, be set when `param` is 2
    /// * `new_owner`- The config's new owner, be set when `param` is 3
    /// * `new_fund_owner`- The config's new fund owner, be set when `param` is 4
    /// * `creator_fee_rate`- The new creator fee rate of amm config, be set when `param` is 7
    /// * `creator_fee_share_rate`- The new share of the creator fee retained by the
    ///   protocol, be set when `param` is 8
    /// * `param`- The value can be 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8, otherwise will report a error
    ///
    pub fn update_amm_config(ctx: Context<UpdateAmmConfig>, param: u8, value: u64) -> Result<()> {
        instructions::update_amm_config(ctx, param, value)
    }

    /// Update pool status for given value
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `status` - The value of status
    ///
    pub fn update_pool_status(ctx: Context<UpdatePoolStatus>, status: u8) -> Result<()> {
        instructions::update_pool_status(ctx, status)
    }

    /// Collect the protocol fee accrued to the pool
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context of accounts
    /// * `amount_0_requested` - The maximum amount of token_0 to send, can be 0 to collect fees in only token_1
    /// * `amount_1_requested` - The maximum amount of token_1 to send, can be 0 to collect fees in only token_0
    ///
    pub fn collect_protocol_fee<'info>(
        ctx: Context<'info, CollectProtocolFee<'info>>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_protocol_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// Hook-aware `collect_protocol_fee`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn collect_protocol_fee_v2<'info>(
        ctx: Context<'info, CollectProtocolFee<'info>>,
        amount_0_requested: u64,
        amount_1_requested: u64,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::collect_protocol_fee_v2(
            ctx,
            amount_0_requested,
            amount_1_requested,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Collect the fund fee accrued to the pool
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context of accounts
    /// * `amount_0_requested` - The maximum amount of token_0 to send, can be 0 to collect fees in only token_1
    /// * `amount_1_requested` - The maximum amount of token_1 to send, can be 0 to collect fees in only token_0
    ///
    pub fn collect_fund_fee<'info>(
        ctx: Context<'info, CollectFundFee<'info>>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_fund_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// Hook-aware `collect_fund_fee`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn collect_fund_fee_v2<'info>(
        ctx: Context<'info, CollectFundFee<'info>>,
        amount_0_requested: u64,
        amount_1_requested: u64,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::collect_fund_fee_v2(
            ctx,
            amount_0_requested,
            amount_1_requested,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Collect the creator fee
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context of accounts
    ///
    pub fn collect_creator_fee<'info>(ctx: Context<'info, CollectCreatorFee<'info>>) -> Result<()> {
        instructions::collect_creator_fee(ctx)
    }

    /// Hook-aware `collect_creator_fee`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn collect_creator_fee_v2<'info>(
        ctx: Context<'info, CollectCreatorFee<'info>>,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::collect_creator_fee_v2(
            ctx,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Collect the creator fee, anyone can call it since the fee is always sent to the
    /// pool creator, the payer only funds the creation of the creator's token accounts.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context of accounts
    ///
    pub fn collect_creator_fee_permissionless<'info>(
        ctx: Context<'info, CollectCreatorFeePermissionless<'info>>,
    ) -> Result<()> {
        instructions::collect_creator_fee_permissionless(ctx)
    }

    /// Hook-aware `collect_creator_fee_permissionless`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn collect_creator_fee_permissionless_v2<'info>(
        ctx: Context<'info, CollectCreatorFeePermissionless<'info>>,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::collect_creator_fee_permissionless_v2(
            ctx,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Create a permission account
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    ///
    pub fn create_permission_pda(ctx: Context<CreatePermissionPda>) -> Result<()> {
        instructions::create_permission_pda(ctx)
    }

    /// Close a permission account
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    ///
    pub fn close_permission_pda(ctx: Context<ClosePermissionPda>) -> Result<()> {
        instructions::close_permission_pda(ctx)
    }

    /// Create a custom creator fee share account for a (creator, amm_config) pair.
    /// While it exists it overrides `AmmConfig::creator_fee_share_rate` when the
    /// creator fee of a pool created by `creator` on `amm_config` is collected.
    /// Must be called by the admin or the creator fee share owner.
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `share_rate` - The share of the creator fee retained by the protocol,
    ///   denominated in hundredths of a bip (10^-6)
    ///
    pub fn create_creator_fee_share(
        ctx: Context<CreateCreatorFeeShare>,
        share_rate: u64,
    ) -> Result<()> {
        instructions::create_creator_fee_share(ctx, share_rate)
    }

    /// Close a custom creator fee share account, the creator fee split falls back to the
    /// rate configured on the amm config afterwards.
    /// Must be called by the admin or the creator fee share owner.
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    ///
    pub fn close_creator_fee_share(ctx: Context<CloseCreatorFeeShare>) -> Result<()> {
        instructions::close_creator_fee_share(ctx)
    }

    /// Creates a pool for the given token pair and the initial price
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `init_amount_0` - the initial amount_0 to deposit
    /// * `init_amount_1` - the initial amount_1 to deposit
    /// * `open_time` - the timestamp allowed for swap
    ///
    pub fn initialize<'info>(
        ctx: Context<'info, Initialize<'info>>,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
    ) -> Result<()> {
        instructions::initialize(ctx, init_amount_0, init_amount_1, open_time)
    }

    /// Hook-aware `initialize`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn initialize_v2<'info>(
        ctx: Context<'info, Initialize<'info>>,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::initialize_v2(
            ctx,
            init_amount_0,
            init_amount_1,
            open_time,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Create a pool with permission
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `init_amount_0` - the initial amount_0 to deposit
    /// * `init_amount_1` - the initial amount_1 to deposit
    /// * `open_time` - the timestamp allowed for swap
    /// * `creator_fee_on` - creator fee model, 0：both token0 and token1 (depends on the input), 1: only token0, 2: only token1
    ///
    pub fn initialize_with_permission<'info>(
        ctx: Context<'info, InitializeWithPermission<'info>>,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
        creator_fee_on: CreatorFeeOn,
    ) -> Result<()> {
        instructions::initialize_with_permission(
            ctx,
            init_amount_0,
            init_amount_1,
            open_time,
            creator_fee_on,
        )
    }

    /// Hook-aware `initialize_with_permission`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn initialize_with_permission_v2<'info>(
        ctx: Context<'info, InitializeWithPermission<'info>>,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
        creator_fee_on: CreatorFeeOn,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::initialize_with_permission_v2(
            ctx,
            init_amount_0,
            init_amount_1,
            open_time,
            creator_fee_on,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Deposit lp token to the pool
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `lp_token_amount` - Increased number of LPs
    /// * `maximum_token_0_amount` -  Maximum token 0 amount to deposit, prevents excessive slippage
    /// * `maximum_token_1_amount` - Maximum token 1 amount to deposit, prevents excessive slippage
    ///
    pub fn deposit<'info>(
        ctx: Context<'info, Deposit<'info>>,
        lp_token_amount: u64,
        maximum_token_0_amount: u64,
        maximum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::deposit(
            ctx,
            lp_token_amount,
            maximum_token_0_amount,
            maximum_token_1_amount,
        )
    }

    /// Hook-aware `deposit`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn deposit_v2<'info>(
        ctx: Context<'info, Deposit<'info>>,
        lp_token_amount: u64,
        maximum_token_0_amount: u64,
        maximum_token_1_amount: u64,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::deposit_v2(
            ctx,
            lp_token_amount,
            maximum_token_0_amount,
            maximum_token_1_amount,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Withdraw lp for token0 and token1
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `lp_token_amount` - Amount of pool tokens to burn. User receives an output of token a and b based on the percentage of the pool tokens that are returned.
    /// * `minimum_token_0_amount` -  Minimum amount of token 0 to receive, prevents excessive slippage
    /// * `minimum_token_1_amount` -  Minimum amount of token 1 to receive, prevents excessive slippage
    ///
    pub fn withdraw<'info>(
        ctx: Context<'info, Withdraw<'info>>,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::withdraw(
            ctx,
            lp_token_amount,
            minimum_token_0_amount,
            minimum_token_1_amount,
        )
    }

    /// Hook-aware `withdraw`. Remaining accounts are the token_0 transfer's hook slice followed by
    /// the token_1 transfer's slice.
    pub fn withdraw_v2<'info>(
        ctx: Context<'info, Withdraw<'info>>,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
        token_0_hook_account_count: u16,
        token_1_hook_account_count: u16,
    ) -> Result<()> {
        instructions::withdraw_v2(
            ctx,
            lp_token_amount,
            minimum_token_0_amount,
            minimum_token_1_amount,
            token_0_hook_account_count,
            token_1_hook_account_count,
        )
    }

    /// Swap the tokens in the pool base input amount
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `amount_in` -  input amount to transfer, output to DESTINATION is based on the exchange rate
    /// * `minimum_amount_out` -  Minimum amount of output token, prevents excessive slippage
    ///
    pub fn swap_base_input<'info>(
        ctx: Context<'info, Swap<'info>>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        instructions::swap_base_input(ctx, amount_in, minimum_amount_out)
    }

    /// Hook-aware exact-input swap. Remaining accounts are framed as the input
    /// transfer slice followed by the output transfer slice.
    pub fn swap_base_input_v2<'info>(
        ctx: Context<'info, Swap<'info>>,
        amount_in: u64,
        minimum_amount_out: u64,
        input_hook_account_count: u16,
        output_hook_account_count: u16,
    ) -> Result<()> {
        instructions::swap_base_input_v2(
            ctx,
            amount_in,
            minimum_amount_out,
            input_hook_account_count,
            output_hook_account_count,
        )
    }

    /// Swap the tokens in the pool base output amount
    ///
    /// # Arguments
    ///
    /// * `ctx`- The context of accounts
    /// * `max_amount_in` -  input amount prevents excessive slippage
    /// * `amount_out` -  amount of output token
    ///
    pub fn swap_base_output<'info>(
        ctx: Context<'info, Swap<'info>>,
        max_amount_in: u64,
        amount_out: u64,
    ) -> Result<()> {
        instructions::swap_base_output(ctx, max_amount_in, amount_out)
    }

    /// Hook-aware exact-output swap. Remaining accounts are framed as the input
    /// transfer slice followed by the output transfer slice.
    pub fn swap_base_output_v2<'info>(
        ctx: Context<'info, Swap<'info>>,
        max_amount_in: u64,
        amount_out: u64,
        input_hook_account_count: u16,
        output_hook_account_count: u16,
    ) -> Result<()> {
        instructions::swap_base_output_v2(
            ctx,
            max_amount_in,
            amount_out,
            input_hook_account_count,
            output_hook_account_count,
        )
    }

    /// Create support token22 mint account which can create pool and send rewards while ignoring unsupported extensions.
    pub fn create_support_mint_associated(ctx: Context<CreateSupportMintAssociated>) -> Result<()> {
        instructions::create_support_mint_associated(ctx)
    }

    /// Close support token22 mint account which can create pool and send rewards while ignoring unsupported extensions.
    pub fn close_support_mint_associated(ctx: Context<CloseSupportMintAssociated>) -> Result<()> {
        instructions::close_support_mint_associated(ctx)
    }

    /// Collect excess lamports, including accounts for SPL tokens owned by authority and Program PDA accounts.
    pub fn collect_excess_lamports<'info>(
        ctx: Context<'info, CollectExcessLamports<'info>>,
    ) -> Result<()> {
        instructions::collect_excess_lamports(ctx)
    }
}

#[cfg(all(test, feature = "integration"))]
mod integration_feature_tests {
    use super::*;

    #[test]
    fn integration_feature_uses_our_program_id_and_admin() {
        assert_eq!(
            ID.to_string(),
            "7tRJH4mmEfNGGLf9E8qEvo3oSjq2b75DhSggb1Wz45fJ"
        );
        let deployer = "QHgnAZswA5wt8ABUv5n7yM4FXFJdNwLsNYXKSVKB1Pm";
        assert_eq!(admin::ID.to_string(), deployer);
        assert_eq!(
            create_pool_fee_reveiver::ID.to_string(),
            "CnoYEaFeS92rnvfY7i1WHY3yKnXgUQqs1xiC1eVj2aYS"
        );
        assert_eq!(protocol_fee_owner::ID.to_string(), deployer);
        assert_eq!(fund_fee_owner::ID.to_string(), deployer);
    }
}
