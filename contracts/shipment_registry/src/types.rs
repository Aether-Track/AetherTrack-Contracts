use soroban_sdk::{contracttype, Address, Bytes, String, Vec};

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ShipmentStatus {
    Created,
    InTransit,
    AtCheckpoint,
    Delivered,
    Disputed,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct LocationProof {
    pub latitude: i64,   // degrees * 1_000_000 (e.g., 40.712776 => 40712776)
    pub longitude: i64,  // degrees * 1_000_000
    pub altitude_m: i32,
    pub timestamp: u64,
    pub reporter: Address,
    pub metadata: String, // JSON: {"speed_kmh": 0, "heading": 0, "accuracy_m": 5}
}

#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub description: String,
    pub location_name: String,
    pub expected_ts: u64,
    pub actual_ts: u64,
    pub confirmed: bool,
    pub confirmed_by: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct MilestoneInput {
    pub description: String,
    pub location_name: String,
    pub expected_ts: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CargoDetails {
    pub description: String,
    pub weight_g: u64,       // grams (avoids floats)
    pub volume_cm3: u64,     // cubic centimetres
    pub is_hazmat: bool,
    pub is_temperature_controlled: bool,
    pub declared_value_stroops: i128,
    pub tracking_number: String,
}

#[contracttype]
#[derive(Clone)]
pub struct Shipment {
    pub id: Bytes,
    pub shipper: Address,
    pub receiver: Address,
    pub carrier: Address,
    pub cargo: CargoDetails,
    pub status: ShipmentStatus,
    pub origin: String,
    pub destination: String,
    pub milestones: Vec<Milestone>,
    pub location_history: Vec<LocationProof>,
    pub created_at: u64,
    pub updated_at: u64,
    pub escrow_contract: Address,
}

#[contracttype]
pub enum DataKey {
    Shipment(Bytes),
    ShipperIndex(Address),
    CarrierIndex(Address),
    ReceiverIndex(Address),
    ShipmentCount,
    Admin,
}
