# AetherTrack — Smart Contracts

Soroban smart contracts powering the AetherTrack real-time shipment tracking platform on the Stellar blockchain.

## Overview

AetherTrack uses two Soroban contracts to bring transparency and programmable logic to international logistics:

| Contract | Responsibility |
|---|---|
| `shipment_registry` | Shipment lifecycle — creation, location proofs, milestone confirmation, custody transfers |
| `payment_escrow` | Milestone-based payment escrow — deposit, release, dispute resolution |

Both contracts are written in Rust targeting the Soroban SDK v21 and compile to WASM for deployment on Stellar.

---

## Architecture

```
AetherTrack-Contracts/
├── Cargo.toml                          # Workspace root
├── Makefile                            # Build / test / deploy shortcuts
├── contracts/
│   ├── shipment_registry/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Contract entry point & logic
│   │       ├── types.rs                # Shipment, Milestone, LocationProof, CargoDetails
│   │       ├── errors.rs               # Typed contract errors
│   │       └── events.rs               # On-chain event emitters
│   └── payment_escrow/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                  # Escrow logic
│           ├── types.rs                # Escrow, MilestonePayment, EscrowStatus
│           ├── errors.rs
│           └── events.rs
└── tests/
    └── shipment_flow_test.rs           # Integration tests
```

---

## ShipmentRegistry Contract

### Storage

| Key | Type | Description |
|---|---|---|
| `Shipment(id)` | `Shipment` | Full shipment record (persistent, 1yr TTL) |
| `ShipperIndex(addr)` | `Vec<Bytes>` | All shipment IDs for a shipper |
| `CarrierIndex(addr)` | `Vec<Bytes>` | All shipment IDs for a carrier |
| `ReceiverIndex(addr)` | `Vec<Bytes>` | All shipment IDs for a receiver |
| `Admin` | `Address` | Contract admin |

### Functions

#### `initialize(admin: Address)`
One-time setup. Sets the admin address. Reverts if already initialized.

#### `create_shipment(...) → Bytes`
Creates a new shipment and returns its unique on-chain ID.

```rust
create_shipment(
    shipper: Address,
    receiver: Address,
    carrier: Address,
    cargo: CargoDetails,
    origin: String,
    destination: String,
    milestones: Vec<MilestoneInput>,  // max 20
    escrow_contract: Address,
) -> Result<Bytes, Error>
```

- Requires `shipper.require_auth()`
- Auto-generates a unique ID from ledger timestamp + sequence + counter
- Emits `CREATED` event

#### `update_location(caller, shipment_id, lat, lon, altitude_m, metadata)`
Records a GPS proof-of-location on-chain.

- Caller must be the shipment's shipper, carrier, or receiver (or admin)
- `lat` / `lon` are stored as `i64` scaled by `1_000_000` (e.g. `6.454° → 6454000`)
- Auto-transitions status `Created → InTransit` on first location update
- Emits `LOCATION` event

#### `confirm_milestone(caller, shipment_id, milestone_index)`
Marks a milestone as confirmed. Only callable by the receiver or admin.

- Emits `MILESTONE` event
- Records the confirming address and actual timestamp

#### `update_status(caller, shipment_id, new_status)`
Updates the overall shipment status. Only shipper or admin.

- Cannot change status of a `Delivered` or `Cancelled` shipment
- Emits `STATUS` event

#### `transfer_custody(caller, shipment_id, new_carrier)`
Hands custody to a new carrier mid-transit. Caller must be the current carrier, shipper, or admin.

- Appends new carrier to the carrier index
- Emits `CUSTODY` event

#### Read-only queries
- `get_shipment(id)` — full shipment struct
- `get_shipments_by_shipper(addr)` — list of IDs
- `get_shipments_by_carrier(addr)` — list of IDs
- `get_shipments_by_receiver(addr)` — list of IDs
- `get_location_history(id)` — all `LocationProof` entries
- `get_admin()` — admin address

### Key Types

```rust
pub struct Shipment {
    pub id: Bytes,
    pub shipper / receiver / carrier: Address,
    pub cargo: CargoDetails,
    pub status: ShipmentStatus,   // Created | InTransit | AtCheckpoint | Delivered | Disputed | Cancelled
    pub origin / destination: String,
    pub milestones: Vec<Milestone>,
    pub location_history: Vec<LocationProof>,
    pub created_at / updated_at: u64,
    pub escrow_contract: Address,
}

pub struct LocationProof {
    pub latitude: i64,      // degrees * 1_000_000
    pub longitude: i64,
    pub altitude_m: i32,
    pub timestamp: u64,
    pub reporter: Address,
    pub metadata: String,   // JSON: speed, heading, accuracy
}
```

---

## PaymentEscrow Contract

One escrow contract is deployed per shipment. It is initialized by the shipper at shipment creation time.

### Functions

#### `initialize(shipment_id, payer, payee, token, total_amount, milestone_amounts, dispute_resolver)`
Sets up the escrow. `milestone_amounts` must sum to `total_amount`.

#### `deposit(amount)`
Payer transfers tokens into the escrow contract via the SAC token interface.

#### `release_milestone(caller, milestone_index)`
Transfers that milestone's payment to the payee. Callable by payer or dispute resolver.

#### `release_all(caller)`
Releases all remaining milestone payments at once (e.g. on full delivery).

#### `raise_dispute(caller)`
Freezes the escrow. Callable by payer or payee.

#### `resolve_dispute(resolver, release_to_payee: bool)`
Dispute resolver sends all remaining locked funds to either the payee (if delivery confirmed) or back to the payer (if refund warranted).

#### `get_escrow()` / `get_balance()`
Read-only queries.

---

## Events

All contract state changes emit Soroban events that the AetherTrack backend indexer listens to:

| Topic | Data | Emitted by |
|---|---|---|
| `CREATED` | `(shipper, carrier)` | `create_shipment` |
| `LOCATION` | `(reporter, lat, lon)` | `update_location` |
| `MILESTONE` | `(index, confirmed_by)` | `confirm_milestone` |
| `STATUS` | `(caller, new_status)` | `update_status` |
| `CUSTODY` | `(from, to)` | `transfer_custody` |
| `FUNDED` | `(payer, amount)` | `deposit` |
| `RELEASED` | `(caller, index, amount)` | `release_milestone` |
| `DISPUTE` | `caller` | `raise_dispute` |
| `RESOLVED` | `(resolver, to_payee)` | `resolve_dispute` |

---

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Soroban CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli): `cargo install --locked soroban-cli`

---

## Getting Started

### Install dependencies & run tests

```bash
# Clone the repo
git clone https://github.com/Aether-Track/AetherTrack-Contracts.git
cd AetherTrack-Contracts

# Run all tests (no wallet or network needed)
make test
```

### Build WASM

```bash
make wasm
# Output: ./artifacts/shipment_registry.wasm
#         ./artifacts/payment_escrow.wasm
```

### Deploy to Testnet

```bash
# Fund a testnet account
soroban keys generate --global deployer --network testnet

export STELLAR_SECRET=$(soroban keys show deployer)

make deploy-testnet
```

After deployment, copy the printed contract IDs into the backend `.env` file:
```
SHIPMENT_REGISTRY_CONTRACT_ID=C...
```

---

## Error Codes

| Code | Name | Description |
|---|---|---|
| 1 | `NotInitialized` | Contract not yet initialized |
| 2 | `AlreadyInitialized` | `initialize` called twice |
| 3 | `Unauthorized` | Caller does not have permission |
| 4 | `ShipmentNotFound` | No shipment with that ID |
| 5 | `InvalidStatus` | Status transition not allowed |
| 6 | `MilestoneNotFound` | Milestone index out of range |
| 7 | `MilestoneAlreadyConfirmed` | Milestone already confirmed |
| 8 | `InvalidLocationData` | Lat/lon out of valid range |
| 9 | `InvalidMilestoneCount` | More than 20 milestones |
| 10 | `ShipmentClosed` | Shipment is Delivered or Cancelled |

---

## Related Repos

- [AetherTrack-Backend](https://github.com/Aether-Track/AetherTrack-Backend) — Node.js API + Stellar event indexer
- [AetherTrack-Frontend](https://github.com/Aether-Track/AetherTrack-Frontend) — Next.js dashboard
