# Security Policy

These contracts move merchants' and customers' money. Please report
vulnerabilities privately.

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/SETTLIX/settlix-contracts/security/advisories/new).
Do **not** open a public issue, pull request, or discussion.

Please include:

- The affected contract and function
- The impact (loss of funds, frozen funds, auth bypass, double payment, ...)
- A reproduction, ideally a failing test using `soroban_sdk::testutils`
- Any suggested fix

We aim to acknowledge reports within 3 business days and to agree a
disclosure timeline with you before any details become public.

## Scope

In scope: everything under `contracts/`, and `scripts/deploy.sh` where it
affects the security of a deployment.

Out of scope: issues in `soroban-sdk`, the Stellar network, or the Stellar
CLI. Report those upstream through the
[Stellar bug bounty](https://stellar.org/grants-and-funding/bug-bounty).

## Supported versions

Only the latest commit on `main` receives security fixes. The contracts have
**not** been externally audited. Do not deploy to mainnet with significant
value before an audit.

## Properties we rely on

Report any way to break these:

1. The payment contract never holds funds. A payment moves the net amount to
   the merchant's settlement address and the fee to the settlement vault in
   the same transaction.
2. An invoice is paid at most once, and a `(merchant, reference)` pair maps to
   at most one invoice.
3. Only an `Active` merchant can create invoices or be paid; suspending a
   merchant stops payment of its open invoices immediately.
4. Fees leave the settlement vault only to the payout wallet the admin
   configured. Nobody can choose another destination at sweep time.
5. Refunds are signed by the account that received the payment, go only to
   the paying customer, and never total more than the invoice amount.
6. Every `initialize` requires the admin's signature, so deployments cannot be
   front-run.
