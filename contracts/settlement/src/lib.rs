//! Settlement: the platform fee vault.
//!
//! The payment contract sends each invoice's platform fee here. Funds can
//! only ever leave to the payout wallet the admin configured, so `sweep` is
//! safe to leave permissionless: anyone (a cron job, a keeper) can trigger
//! it, but nobody can choose where the money goes. This contract is also the
//! single source of truth for the fee rate, which the payment contract reads.

#![no_std]

use shared::{extend_instance, MAX_FEE_BPS};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SettlementError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidFee = 3,
    NothingToSweep = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub payout_wallet: Address,
    pub fee_bps: u32,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesSwept {
    #[topic]
    pub token: Address,
    pub amount: i128,
    pub to: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementConfigured {
    pub fee_bps: u32,
    pub payout_wallet: Address,
}

soroban_sdk::contractmeta!(
    key = "source_repo",
    val = "github:SETTLIX/settlix-contracts"
);
soroban_sdk::contractmeta!(key = "desc", val = "Settlix platform fee vault");

#[contract]
pub struct SettlementContract;

#[contractimpl]
impl SettlementContract {
    /// One-time setup. The admin signs, so nobody can front-run
    /// initialization with their own wallet.
    pub fn initialize(
        env: Env,
        admin: Address,
        payout_wallet: Address,
        fee_bps: u32,
    ) -> Result<(), SettlementError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(SettlementError::AlreadyInitialized);
        }
        admin.require_auth();
        if fee_bps > MAX_FEE_BPS {
            return Err(SettlementError::InvalidFee);
        }
        Self::save(
            &env,
            &Config {
                admin,
                payout_wallet,
                fee_bps,
            },
        );
        Ok(())
    }

    /// Sends this contract's whole balance of `token` to the payout wallet.
    /// Permissionless: the destination is fixed by the admin.
    pub fn sweep(env: Env, token_id: Address) -> Result<i128, SettlementError> {
        let config = Self::config(&env)?;
        let token = token::Client::new(&env, &token_id);
        let amount = token.balance(&env.current_contract_address());
        if amount <= 0 {
            return Err(SettlementError::NothingToSweep);
        }
        token.transfer(
            &env.current_contract_address(),
            &config.payout_wallet,
            &amount,
        );
        FeesSwept {
            token: token_id,
            amount,
            to: config.payout_wallet,
        }
        .publish(&env);
        Ok(amount)
    }

    pub fn set_fee_bps(env: Env, fee_bps: u32) -> Result<(), SettlementError> {
        if fee_bps > MAX_FEE_BPS {
            return Err(SettlementError::InvalidFee);
        }
        let mut config = Self::config(&env)?;
        config.admin.require_auth();
        config.fee_bps = fee_bps;
        Self::save(&env, &config);
        Ok(())
    }

    /// Changing where fees go needs the admin and the new wallet to sign,
    /// so a typo cannot send fees to an address nobody controls.
    pub fn set_payout_wallet(env: Env, wallet: Address) -> Result<(), SettlementError> {
        let mut config = Self::config(&env)?;
        config.admin.require_auth();
        wallet.require_auth();
        config.payout_wallet = wallet;
        Self::save(&env, &config);
        Ok(())
    }

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), SettlementError> {
        let mut config = Self::config(&env)?;
        config.admin.require_auth();
        new_admin.require_auth();
        config.admin = new_admin;
        Self::save(&env, &config);
        Ok(())
    }

    pub fn fee_bps(env: Env) -> u32 {
        Self::config(&env).map(|c| c.fee_bps).unwrap_or(0)
    }

    pub fn get_config(env: Env) -> Result<Config, SettlementError> {
        Self::config(&env)
    }

    pub fn balance(env: Env, token_id: Address) -> i128 {
        token::Client::new(&env, &token_id).balance(&env.current_contract_address())
    }

    fn config(env: &Env) -> Result<Config, SettlementError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(SettlementError::NotInitialized)
    }

    fn save(env: &Env, config: &Config) {
        env.storage().instance().set(&DataKey::Config, config);
        extend_instance(env);
        SettlementConfigured {
            fee_bps: config.fee_bps,
            payout_wallet: config.payout_wallet.clone(),
        }
        .publish(env);
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Env};

    fn setup() -> (Env, SettlementContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let c = SettlementContractClient::new(&env, &env.register(SettlementContract, ()));
        let wallet = Address::generate(&env);
        c.initialize(&Address::generate(&env), &wallet, &50);
        let asset = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        (env, c, wallet, asset)
    }

    #[test]
    fn sweep_pays_only_the_configured_wallet() {
        let (env, c, wallet, asset) = setup();
        StellarAssetClient::new(&env, &asset).mint(&c.address, &1_234);
        assert_eq!(c.sweep(&asset), 1_234);
        assert_eq!(token::Client::new(&env, &asset).balance(&wallet), 1_234);
        assert_eq!(
            c.try_sweep(&asset),
            Err(Ok(SettlementError::NothingToSweep))
        );
    }

    #[test]
    fn fee_is_capped_and_readable() {
        let (_env, c, _wallet, _asset) = setup();
        assert_eq!(c.fee_bps(), 50);
        assert_eq!(
            c.try_set_fee_bps(&(MAX_FEE_BPS + 1)),
            Err(Ok(SettlementError::InvalidFee))
        );
        c.set_fee_bps(&100);
        assert_eq!(c.fee_bps(), 100);
    }

    #[test]
    fn initialize_once() {
        let (env, c, wallet, _asset) = setup();
        assert_eq!(
            c.try_initialize(&Address::generate(&env), &wallet, &0),
            Err(Ok(SettlementError::AlreadyInitialized))
        );
    }

    #[test]
    fn payout_wallet_change_moves_future_sweeps() {
        let (env, c, _old, asset) = setup();
        let new_wallet = Address::generate(&env);
        c.set_payout_wallet(&new_wallet);
        StellarAssetClient::new(&env, &asset).mint(&c.address, &10);
        c.sweep(&asset);
        assert_eq!(token::Client::new(&env, &asset).balance(&new_wallet), 10);
    }
}
