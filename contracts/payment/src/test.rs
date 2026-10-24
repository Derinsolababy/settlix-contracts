extern crate std;

use super::*;
use merchant_registry_contract::{MerchantRegistryContract, MerchantRegistryContractClient};
use settlement_contract::{SettlementContract, SettlementContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    BytesN, Env, String,
};

const T0: u64 = 1_000_000;
const HOUR: u64 = 3_600;

struct World<'a> {
    env: Env,
    pay: PaymentContractClient<'a>,
    registry: MerchantRegistryContractClient<'a>,
    settlement: SettlementContractClient<'a>,
    usdc: Address,
    token: token::Client<'a>,
    merchant: Address,
    payout: Address,
    customer: Address,
}

/// Registry with one approved merchant, a 0.5% fee vault, USDC allowed,
/// and a customer holding 1,000 USDC (7 decimals).
fn world() -> World<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = T0);
    let admin = Address::generate(&env);

    let registry =
        MerchantRegistryContractClient::new(&env, &env.register(MerchantRegistryContract, ()));
    registry.initialize(&admin);
    let settlement = SettlementContractClient::new(&env, &env.register(SettlementContract, ()));
    settlement.initialize(&admin, &Address::generate(&env), &50);
    let pay = PaymentContractClient::new(&env, &env.register(PaymentContract, ()));
    pay.initialize(&admin, &registry.address, &settlement.address);

    let usdc = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    registry.set_token_allowed(&usdc, &true);
    let merchant = Address::generate(&env);
    let payout = Address::generate(&env);
    registry.apply(
        &merchant,
        &String::from_str(&env, "Ada's Fabrics"),
        &BytesN::from_array(&env, &[1; 32]),
        &payout,
    );
    registry.approve(&merchant);
    let customer = Address::generate(&env);
    StellarAssetClient::new(&env, &usdc).mint(&customer, &10_000_000_000);

    World {
        token: token::Client::new(&env, &usdc),
        env,
        pay,
        registry,
        settlement,
        usdc,
        merchant,
        payout,
        customer,
    }
}

fn invoice(w: &World, amount: i128, reference: &str) -> u64 {
    w.pay.create_invoice(
        &w.merchant,
        &w.usdc,
        &amount,
        &String::from_str(&w.env, reference),
        &(T0 + HOUR),
    )
}

#[test]
fn paying_an_invoice_splits_fee_and_net_atomically() {
    let w = world();
    let id = invoice(&w, 100_000_000, "INV-001"); // 10 USDC
    w.pay.pay_invoice(&w.customer, &id);

    assert_eq!(w.token.balance(&w.payout), 99_500_000);
    assert_eq!(w.token.balance(&w.settlement.address), 500_000);
    assert_eq!(w.token.balance(&w.pay.address), 0); // never custodies funds
    let inv = w.pay.get_invoice(&id);
    assert_eq!(inv.status, InvoiceStatus::Paid);
    assert_eq!(inv.customer, Some(w.customer.clone()));
    assert_eq!(inv.fee, 500_000);

    // Fees reach the platform wallet only through the vault.
    assert_eq!(w.settlement.sweep(&w.usdc), 500_000);
}

#[test]
fn an_invoice_is_paid_once_and_references_are_unique() {
    let w = world();
    let id = invoice(&w, 1_000, "INV-002");
    w.pay.pay_invoice(&w.customer, &id);
    assert_eq!(
        w.pay.try_pay_invoice(&w.customer, &id),
        Err(Ok(PaymentError::NotOpen))
    );
    assert_eq!(
        w.pay.try_create_invoice(
            &w.merchant,
            &w.usdc,
            &1,
            &String::from_str(&w.env, "INV-002"),
            &(T0 + HOUR)
        ),
        Err(Ok(PaymentError::DuplicateReference))
    );
    assert_eq!(
        w.pay
            .invoice_by_reference(&w.merchant, &String::from_str(&w.env, "INV-002"))
            .id,
        id
    );
}

#[test]
fn expired_and_cancelled_invoices_cannot_be_paid() {
    let w = world();
    let a = invoice(&w, 1_000, "A");
    let b = invoice(&w, 1_000, "B");
    w.pay.cancel_invoice(&b);
    assert_eq!(
        w.pay.try_pay_invoice(&w.customer, &b),
        Err(Ok(PaymentError::NotOpen))
    );
    w.env.ledger().with_mut(|l| l.timestamp = T0 + HOUR);
    assert_eq!(
        w.pay.try_pay_invoice(&w.customer, &a),
        Err(Ok(PaymentError::Expired))
    );
    assert_eq!(w.token.balance(&w.customer), 10_000_000_000);
}

#[test]
fn suspended_merchants_cannot_invoice_or_be_paid() {
    let w = world();
    let id = invoice(&w, 1_000, "S");
    w.registry.suspend(&w.merchant);
    assert_eq!(
        w.pay.try_pay_invoice(&w.customer, &id),
        Err(Ok(PaymentError::MerchantNotActive))
    );
    assert_eq!(
        w.pay.try_create_invoice(
            &w.merchant,
            &w.usdc,
            &1,
            &String::from_str(&w.env, "S2"),
            &(T0 + HOUR)
        ),
        Err(Ok(PaymentError::MerchantNotActive))
    );
}

#[test]
fn only_allowed_tokens_can_be_invoiced() {
    let w = world();
    let other = w
        .env
        .register_stellar_asset_contract_v2(Address::generate(&w.env))
        .address();
    assert_eq!(
        w.pay.try_create_invoice(
            &w.merchant,
            &other,
            &1,
            &String::from_str(&w.env, "T"),
            &(T0 + HOUR)
        ),
        Err(Ok(PaymentError::TokenNotAllowed))
    );
}

#[test]
fn payouts_follow_the_current_settlement_address() {
    let w = world();
    let id = invoice(&w, 10_000, "P");
    let new_payout = Address::generate(&w.env);
    w.registry.set_settlement_address(&w.merchant, &new_payout);
    w.pay.pay_invoice(&w.customer, &id);
    assert_eq!(w.token.balance(&new_payout), 9_950);
    assert_eq!(w.pay.get_invoice(&id).payee, Some(new_payout));
}

#[test]
fn partial_then_full_refund_from_the_payee() {
    let w = world();
    let id = invoice(&w, 10_000, "R");
    w.pay.pay_invoice(&w.customer, &id);
    assert_eq!(w.pay.refund(&id, &4_000), 4_000);
    assert_eq!(
        w.pay.get_invoice(&id).status,
        InvoiceStatus::PartiallyRefunded
    );
    // Refunds can exceed what the payee received (fee was taken), up to the invoice amount.
    StellarAssetClient::new(&w.env, &w.usdc).mint(&w.payout, &50);
    w.pay.refund(&id, &6_000);
    assert_eq!(w.pay.get_invoice(&id).status, InvoiceStatus::Refunded);
    assert_eq!(w.pay.try_refund(&id, &1), Err(Ok(PaymentError::NotOpen)));
}

#[test]
fn refunds_are_capped_and_only_for_paid_invoices() {
    let w = world();
    let open = invoice(&w, 10_000, "O");
    assert_eq!(w.pay.try_refund(&open, &1), Err(Ok(PaymentError::NotOpen)));
    w.pay.pay_invoice(&w.customer, &open);
    assert_eq!(
        w.pay.try_refund(&open, &10_001),
        Err(Ok(PaymentError::RefundTooLarge))
    );
}

#[test]
fn input_validation_and_pause() {
    let w = world();
    let r = |s: &str| String::from_str(&w.env, s);
    assert_eq!(
        w.pay
            .try_create_invoice(&w.merchant, &w.usdc, &0, &r("Z"), &(T0 + HOUR)),
        Err(Ok(PaymentError::InvalidAmount))
    );
    assert_eq!(
        w.pay
            .try_create_invoice(&w.merchant, &w.usdc, &1, &r(""), &(T0 + HOUR)),
        Err(Ok(PaymentError::InvalidReference))
    );
    assert_eq!(
        w.pay
            .try_create_invoice(&w.merchant, &w.usdc, &1, &r("E"), &T0),
        Err(Ok(PaymentError::InvalidExpiry))
    );
    w.pay.pause();
    assert_eq!(
        w.pay
            .try_create_invoice(&w.merchant, &w.usdc, &1, &r("P"), &(T0 + HOUR)),
        Err(Ok(PaymentError::Paused))
    );
    w.pay.unpause();
    invoice(&w, 1, "P");
}
