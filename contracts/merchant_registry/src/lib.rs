#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Merchant(Address),
    Admin,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merchant {
    pub address: Address,
    pub name: String,
    pub business_id: String,
    pub settlement_address: Address,
    pub is_active: bool,
    pub registered_at: u64,
}

#[contract]
pub struct MerchantRegistryContract;

#[contractimpl]
impl MerchantRegistryContract {
    /// Initialize contract with admin
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        
        env.storage().instance().set(&DataKey::Admin, &admin);
    }
    
    /// Register a new merchant
    pub fn register_merchant(
        env: Env,
        merchant: Address,
        name: String,
        business_id: String,
        settlement_address: Address,
    ) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        
        let merchant_data = Merchant {
            address: merchant.clone(),
            name: name.clone(),
            business_id,
            settlement_address,
            is_active: true,
            registered_at: env.ledger().timestamp(),
        };
        
        env.storage().persistent().set(&DataKey::Merchant(merchant.clone()), &merchant_data);
        
        env.events().publish(
            (String::from_str(&env, "merchant_registered"),),
            (merchant, name)
        );
    }
    
    /// Get merchant details
    pub fn get_merchant(env: Env, merchant: Address) -> Merchant {
        env.storage()
            .persistent()
            .get(&DataKey::Merchant(merchant))
            .expect("Merchant not found")
    }
    
    /// Update merchant status
    pub fn set_merchant_status(env: Env, merchant: Address, is_active: bool) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        
        let mut merchant_data: Merchant = env.storage()
            .persistent()
            .get(&DataKey::Merchant(merchant.clone()))
            .expect("Merchant not found");
        
        merchant_data.is_active = is_active;
        env.storage().persistent().set(&DataKey::Merchant(merchant.clone()), &merchant_data);
        
        env.events().publish(
            (String::from_str(&env, "merchant_status_updated"),),
            (merchant, is_active)
        );
    }
    
    /// Check if merchant is active
    pub fn is_merchant_active(env: Env, merchant: Address) -> bool {
        let merchant_data: Merchant = env.storage()
            .persistent()
            .get(&DataKey::Merchant(merchant))
            .expect("Merchant not found");
        
        merchant_data.is_active
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_register_merchant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MerchantRegistryContract);
        let client = MerchantRegistryContractClient::new(&env, &contract_id);
        
        let admin = Address::generate(&env);
        let merchant = Address::generate(&env);
        let settlement = Address::generate(&env);
        
        client.initialize(&admin);
        
        client.register_merchant(
            &merchant,
            &String::from_str(&env, "Coffee Shop"),
            &String::from_str(&env, "BIZ-123"),
            &settlement,
        );
        
        let merchant_data = client.get_merchant(&merchant);
        assert_eq!(merchant_data.is_active, true);
    }
}
