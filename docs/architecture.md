# Architecture

Three contracts with one job each, plus a shared library crate.

```
                 +--------------------+
                 | merchant_registry  |  who may be paid, where, in which tokens
                 +---------^----------+
                           | payee(), is_token_allowed()
+----------+  pay_invoice  +-----------+  fee_bps()  +------------+
| customer |-------------->|  payment  |------------>| settlement |
+----------+               +-----------+             +-----+------+
      |  net to merchant's settlement address              | sweep()
      |  fee to the settlement contract                    v
      +--------------------------------------------- payout wallet
```

The payment contract calls the other two through `#[contractclient]`
interfaces, so they can be audited and upgraded independently.

## payment

| Function | Who signs | What it does |
| --- | --- | --- |
| `initialize(admin, registry, settlement)` | admin | One-time setup |
| `create_invoice(merchant, token, amount, reference, expires_at) -> u64` | merchant | Opens an invoice. Merchant must be `Active`, token allow-listed, reference unique for the merchant (1-64 bytes), expiry in the future |
| `pay_invoice(customer, id)` | customer | Re-checks the merchant, reads the fee rate, transfers net and fee, marks `Paid` |
| `cancel_invoice(id)` | merchant | `Open` to `Cancelled` |
| `refund(id, amount) -> i128` | the payee recorded on the invoice | Sends `amount` back to the customer; total refunds are capped at the invoice amount |
| `pause()` / `unpause()` | admin | Blocks new invoices and payments; refunds and cancellations still work |
| `get_invoice(id)`, `invoice_by_reference(merchant, reference)`, `invoice_count()`, `get_config()` | none | Views |

Invoice status: `Open` to `Paid` or `Cancelled`; `Paid` to
`PartiallyRefunded` to `Refunded`. An expired invoice stays `Open` but can no
longer be paid.

The payee is recorded at payment time, so a merchant who later moves its
settlement address still refunds from the account that actually received the
money. The fee is not returned on refund.

## merchant_registry

| Function | Who signs | What it does |
| --- | --- | --- |
| `initialize(admin)` | admin | One-time setup |
| `apply(merchant, name, business_hash, settlement_address)` | merchant | Creates a `Pending` record. `business_hash` is the SHA-256 of the business registration ID |
| `approve` / `suspend` / `reinstate(merchant)` | admin | `Pending` to `Active`; `Active` to/from `Suspended` |
| `reject(merchant)` | admin | Deletes a pending application so it can be corrected and resubmitted |
| `set_settlement_address(merchant, address)` | merchant and new address | Moves future payouts |
| `set_token_allowed(token, allowed)` | admin | Maintains the allow-list |
| `transfer_admin(new_admin)` | both admins | Hands over control |
| `get_merchant`, `payee`, `is_token_allowed` | none | Views; `payee` is `None` unless `Active` |

## settlement

| Function | Who signs | What it does |
| --- | --- | --- |
| `initialize(admin, payout_wallet, fee_bps)` | admin | One-time setup; `fee_bps` at most 1000 (10%) |
| `sweep(token) -> i128` | anyone | Sends the vault's whole balance of `token` to the payout wallet |
| `set_fee_bps(fee_bps)` | admin | Changes the rate for future payments |
| `set_payout_wallet(wallet)` | admin and new wallet | Changes where sweeps go |
| `transfer_admin(new_admin)` | both admins | Hands over control |
| `fee_bps`, `get_config`, `balance(token)` | none | Views |

## Fee math

`shared::split_fee(amount, bps)` returns `(fee, net)` with
`fee = amount * bps / 10_000`, rounded down, and `net = amount - fee`. Both
use checked arithmetic, so `fee + net == amount` always and overflow is an
error rather than a wrap.

## Storage

Configuration lives in instance storage. Merchants, invoices, references and
the token allow-list live in persistent storage. Every read or write extends
the entry's TTL: 30 days for instance storage, 90 days for persistent
entries, topped back up whenever more than a day of it has elapsed.

## Events

| Contract | Event | Topics | Data |
| --- | --- | --- | --- |
| payment | `invoice_created` | merchant, id | token, amount, reference, expires_at |
| payment | `invoice_paid` | merchant, id | customer, amount, fee |
| payment | `invoice_cancelled` | id | |
| payment | `invoice_refunded` | id | amount, total_refunded |
| registry | `merchant_applied` | merchant | name |
| registry | `merchant_status_changed` | merchant | status |
| registry | `token_allowed` | token | allowed |
| settlement | `fees_swept` | token | amount, to |
| settlement | `settlement_configured` | | fee_bps, payout_wallet |

## Error codes

| Code | payment | merchant_registry | settlement |
| --- | --- | --- | --- |
| 1 | AlreadyInitialized | AlreadyInitialized | AlreadyInitialized |
| 2 | NotInitialized | NotInitialized | NotInitialized |
| 3 | Paused | AlreadyApplied | InvalidFee |
| 4 | InvalidAmount | MerchantNotFound | NothingToSweep |
| 5 | InvalidReference | InvalidName | |
| 6 | DuplicateReference | InvalidTransition | |
| 7 | MerchantNotActive | | |
| 8 | TokenNotAllowed | | |
| 9 | InvoiceNotFound | | |
| 10 | NotOpen | | |
| 11 | Expired | | |
| 12 | InvalidExpiry | | |
| 13 | RefundTooLarge | | |
| 14 | Overflow | | |

Codes are part of the public interface. New variants get new numbers;
existing ones are never renumbered.
