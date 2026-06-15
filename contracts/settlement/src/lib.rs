#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Settlement(u64),
    MerchantBalance(Address),
    Counter,
    PlatformFee,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settlement {
    pub id: u64,
    pub merchant: Address,
    pub amount: i128,
    pub fee: i128,
    pub net_amount: i128,
    pub token: Address,
    pub status: SettlementStatus,
    pub created_at: u64,
    pub settled_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Pending,
    Completed,
    Failed,
}

#[contract]
pub struct SettlementContract;

#[contractimpl]
impl SettlementContract {
    /// Initialize with platform fee (in basis points, e.g., 50 = 0.5%)
    pub fn initialize(env: Env, platform_fee: u32) {
        if env.storage().instance().has(&DataKey::PlatformFee) {
            panic!("Already initialized");
        }
        
        env.storage().instance().set(&DataKey::PlatformFee, &platform_fee);
    }
    
    /// Create settlement for merchant
    pub fn create_settlement(
        env: Env,
        merchant: Address,
        amount: i128,
        token: Address,
    ) -> u64 {
        merchant.require_auth();
        
        let platform_fee: u32 = env.storage().instance().get(&DataKey::PlatformFee).unwrap_or(50);
        
        // Calculate fee (amount * platform_fee / 10000)
        let fee = (amount * platform_fee as i128) / 10000;
        let net_amount = amount - fee;
        
        let counter = env.storage().instance().get(&DataKey::Counter).unwrap_or(0u64);
        let settlement_id = counter + 1;
        
        let settlement = Settlement {
            id: settlement_id,
            merchant: merchant.clone(),
            amount,
            fee,
            net_amount,
            token: token.clone(),
            status: SettlementStatus::Pending,
            created_at: env.ledger().timestamp(),
            settled_at: None,
        };
        
        env.storage().persistent().set(&DataKey::Settlement(settlement_id), &settlement);
        env.storage().instance().set(&DataKey::Counter, &settlement_id);
        
        env.events().publish(
            (String::from_str(&env, "settlement_created"),),
            (settlement_id, merchant, net_amount)
        );
        
        settlement_id
    }
    
    /// Execute settlement payout
    pub fn execute_settlement(
        env: Env,
        settlement_id: u64,
        platform_wallet: Address,
    ) -> bool {
        let mut settlement: Settlement = env.storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .expect("Settlement not found");
        
        if settlement.status != SettlementStatus::Pending {
            panic!("Settlement already processed");
        }
        
        let token_client = token::Client::new(&env, &settlement.token);
        let contract_address = env.current_contract_address();
        
        // Transfer net amount to merchant
        token_client.transfer(
            &contract_address,
            &settlement.merchant,
            &settlement.net_amount,
        );
        
        // Transfer fee to platform
        token_client.transfer(
            &contract_address,
            &platform_wallet,
            &settlement.fee,
        );
        
        // Update settlement
        settlement.status = SettlementStatus::Completed;
        settlement.settled_at = Some(env.ledger().timestamp());
        
        env.storage().persistent().set(&DataKey::Settlement(settlement_id), &settlement);
        
        env.events().publish(
            (String::from_str(&env, "settlement_completed"),),
            (settlement_id, settlement.merchant.clone())
        );
        
        true
    }
    
    /// Get settlement details
    pub fn get_settlement(env: Env, settlement_id: u64) -> Settlement {
        env.storage()
            .persistent()
            .get(&DataKey::Settlement(settlement_id))
            .expect("Settlement not found")
    }
    
    /// Get platform fee
    pub fn get_platform_fee(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::PlatformFee).unwrap_or(50)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_settlement_with_fee() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SettlementContract);
        let client = SettlementContractClient::new(&env, &contract_id);
        
        client.initialize(&50); // 0.5% fee
        
        let merchant = Address::generate(&env);
        let token = Address::generate(&env);
        
        let settlement_id = client.create_settlement(&merchant, &10000, &token);
        let settlement = client.get_settlement(&settlement_id);
        
        assert_eq!(settlement.amount, 10000);
        assert_eq!(settlement.fee, 50); // 0.5% of 10000
        assert_eq!(settlement.net_amount, 9950);
    }
}
