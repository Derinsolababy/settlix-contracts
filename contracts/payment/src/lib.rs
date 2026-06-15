#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Vec};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Payment(u64),      // Payment ID -> Payment details
    MerchantPayments(Address), // Merchant -> Payment IDs
    Counter,           // Global payment counter
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payment {
    pub id: u64,
    pub merchant: Address,
    pub customer: Address,
    pub amount: i128,
    pub token: Address,
    pub status: PaymentStatus,
    pub timestamp: u64,
    pub reference: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Refunded,
    Failed,
}

#[contract]
pub struct PaymentContract;

#[contractimpl]
impl PaymentContract {
    /// Initialize a payment from customer to merchant
    pub fn create_payment(
        env: Env,
        customer: Address,
        merchant: Address,
        amount: i128,
        token: Address,
        reference: String,
    ) -> u64 {
        customer.require_auth();
        
        // Get next payment ID
        let counter = env.storage().instance().get(&DataKey::Counter).unwrap_or(0u64);
        let payment_id = counter + 1;
        
        // Create payment record
        let payment = Payment {
            id: payment_id,
            merchant: merchant.clone(),
            customer: customer.clone(),
            amount,
            token: token.clone(),
            status: PaymentStatus::Pending,
            timestamp: env.ledger().timestamp(),
            reference,
        };
        
        // Store payment
        env.storage().persistent().set(&DataKey::Payment(payment_id), &payment);
        
        // Update counter
        env.storage().instance().set(&DataKey::Counter, &payment_id);
        
        // Emit event
        env.events().publish(
            (String::from_str(&env, "payment_created"),),
            (payment_id, merchant.clone(), customer.clone(), amount)
        );
        
        payment_id
    }
    
    /// Execute payment transfer
    pub fn execute_payment(env: Env, payment_id: u64) -> bool {
        let mut payment: Payment = env.storage()
            .persistent()
            .get(&DataKey::Payment(payment_id))
            .expect("Payment not found");
        
        // Only execute pending payments
        if payment.status != PaymentStatus::Pending {
            panic!("Payment already processed");
        }
        
        payment.customer.require_auth();
        
        // Transfer tokens from customer to merchant
        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(&payment.customer, &payment.merchant, &payment.amount);
        
        // Update payment status
        payment.status = PaymentStatus::Completed;
        env.storage().persistent().set(&DataKey::Payment(payment_id), &payment);
        
        // Emit event
        env.events().publish(
            (String::from_str(&env, "payment_completed"),),
            (payment_id, payment.merchant.clone(), payment.amount)
        );
        
        true
    }
    
    /// Get payment details
    pub fn get_payment(env: Env, payment_id: u64) -> Payment {
        env.storage()
            .persistent()
            .get(&DataKey::Payment(payment_id))
            .expect("Payment not found")
    }
    
    /// Refund a payment
    pub fn refund_payment(env: Env, payment_id: u64) -> bool {
        let mut payment: Payment = env.storage()
            .persistent()
            .get(&DataKey::Payment(payment_id))
            .expect("Payment not found");
        
        // Only refund completed payments
        if payment.status != PaymentStatus::Completed {
            panic!("Can only refund completed payments");
        }
        
        payment.merchant.require_auth();
        
        // Transfer tokens back from merchant to customer
        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(&payment.merchant, &payment.customer, &payment.amount);
        
        // Update payment status
        payment.status = PaymentStatus::Refunded;
        env.storage().persistent().set(&DataKey::Payment(payment_id), &payment);
        
        // Emit event
        env.events().publish(
            (String::from_str(&env, "payment_refunded"),),
            (payment_id, payment.customer.clone(), payment.amount)
        );
        
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_create_payment() {
        let env = Env::default();
        let contract_id = env.register_contract(None, PaymentContract);
        let client = PaymentContractClient::new(&env, &contract_id);
        
        let merchant = Address::generate(&env);
        let customer = Address::generate(&env);
        let token = Address::generate(&env);
        let reference = String::from_str(&env, "ORDER-123");
        
        let payment_id = client.create_payment(&customer, &merchant, &100, &token, &reference);
        
        assert_eq!(payment_id, 1);
        
        let payment = client.get_payment(&payment_id);
        assert_eq!(payment.amount, 100);
        assert_eq!(payment.status, PaymentStatus::Pending);
    }
}
