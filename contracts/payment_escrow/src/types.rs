use soroban_sdk::{contracttype, Address, Bytes, Vec};

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum EscrowStatus {
    Pending,   // waiting for deposit
    Active,    // funded, shipment in progress
    PartiallyReleased,
    FullyReleased,
    Refunded,
    Disputed,
}

#[contracttype]
#[derive(Clone)]
pub struct MilestonePayment {
    pub amount_stroops: i128,
    pub released: bool,
    pub released_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct Escrow {
    pub shipment_id: Bytes,
    pub payer: Address,
    pub payee: Address,
    pub token: Address,          // SAC token address (USDC, XLM, etc.)
    pub total_amount: i128,
    pub deposited_amount: i128,
    pub milestone_payments: Vec<MilestonePayment>,
    pub status: EscrowStatus,
    pub dispute_resolver: Address,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contracttype]
pub enum DataKey {
    Escrow,
    Admin,
}
