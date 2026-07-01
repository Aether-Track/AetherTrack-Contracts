#![cfg(test)]

extern crate std;

use shipment_registry::{
    types::{CargoDetails, MilestoneInput, ShipmentStatus},
    ShipmentRegistryContract, ShipmentRegistryContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, Bytes, Env, IntoVal, String,
};

fn setup_env() -> (Env, Address, ShipmentRegistryContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ShipmentRegistryContract);
    let client = ShipmentRegistryContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, client)
}

fn make_cargo(env: &Env) -> CargoDetails {
    CargoDetails {
        description: String::from_str(env, "Electronics — 50 units"),
        weight_g: 25_000,
        volume_cm3: 50_000,
        is_hazmat: false,
        is_temperature_controlled: false,
        declared_value_stroops: 10_000_000_000,
        tracking_number: String::from_str(env, "TRK-2024-001"),
    }
}

fn make_milestones(env: &Env) -> soroban_sdk::Vec<MilestoneInput> {
    vec![
        env,
        MilestoneInput {
            description: String::from_str(env, "Pickup from warehouse"),
            location_name: String::from_str(env, "Lagos, Nigeria"),
            expected_ts: 1_700_000_000,
        },
        MilestoneInput {
            description: String::from_str(env, "Port clearance"),
            location_name: String::from_str(env, "Apapa Port, Lagos"),
            expected_ts: 1_700_086_400,
        },
        MilestoneInput {
            description: String::from_str(env, "Delivery to destination"),
            location_name: String::from_str(env, "Nairobi, Kenya"),
            expected_ts: 1_700_432_000,
        },
    ]
}

#[test]
fn test_create_shipment() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let escrow = Address::generate(&env);
    let cargo = make_cargo(&env);
    let milestones = make_milestones(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &cargo,
            &String::from_str(&env, "Lagos, Nigeria"),
            &String::from_str(&env, "Nairobi, Kenya"),
            &milestones,
            &escrow,
        )
        .unwrap();

    let shipment = client.get_shipment(&id).unwrap();
    assert_eq!(shipment.shipper, shipper);
    assert_eq!(shipment.carrier, carrier);
    assert_eq!(shipment.receiver, receiver);
    assert_eq!(shipment.milestones.len(), 3);
    assert!(matches!(shipment.status, ShipmentStatus::Created));
}

#[test]
fn test_update_location_and_status_transition() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let escrow = Address::generate(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    // First location update should move status -> InTransit
    client
        .update_location(
            &carrier,
            &id,
            &6_454_000i64,   // 6.454° N
            &3_396_000i64,   // 3.396° E
            &0i32,
            &String::from_str(&env, r#"{"speed_kmh":60,"heading":90}"#),
        )
        .unwrap();

    let shipment = client.get_shipment(&id).unwrap();
    assert!(matches!(shipment.status, ShipmentStatus::InTransit));
    assert_eq!(shipment.location_history.len(), 1);
}

#[test]
fn test_confirm_milestone() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let escrow = Address::generate(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    // Receiver confirms milestone 0
    client.confirm_milestone(&receiver, &id, &0).unwrap();

    let shipment = client.get_shipment(&id).unwrap();
    let m0 = shipment.milestones.get(0).unwrap();
    assert!(m0.confirmed);
    assert_eq!(m0.confirmed_by, receiver);
}

#[test]
fn test_transfer_custody() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier_a = Address::generate(&env);
    let carrier_b = Address::generate(&env);
    let escrow = Address::generate(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier_a,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    client.transfer_custody(&carrier_a, &id, &carrier_b).unwrap();

    let shipment = client.get_shipment(&id).unwrap();
    assert_eq!(shipment.carrier, carrier_b);
}

#[test]
fn test_unauthorized_location_update_rejected() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let stranger = Address::generate(&env);
    let escrow = Address::generate(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    let result = client.try_update_location(
        &stranger,
        &id,
        &6_454_000i64,
        &3_396_000i64,
        &0i32,
        &String::from_str(&env, "{}"),
    );
    assert!(result.is_err());
}

#[test]
fn test_duplicate_milestone_confirm_rejected() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let escrow = Address::generate(&env);

    let id = client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    client.confirm_milestone(&receiver, &id, &0).unwrap();

    let result = client.try_confirm_milestone(&receiver, &id, &0);
    assert!(result.is_err());
}

#[test]
fn test_index_queries() {
    let (env, _admin, client) = setup_env();

    let shipper = Address::generate(&env);
    let receiver = Address::generate(&env);
    let carrier = Address::generate(&env);
    let escrow = Address::generate(&env);

    // Create two shipments for the same shipper
    client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Lagos"),
            &String::from_str(&env, "Nairobi"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    env.ledger().with_mut(|l| l.sequence_number += 1);

    client
        .create_shipment(
            &shipper,
            &receiver,
            &carrier,
            &make_cargo(&env),
            &String::from_str(&env, "Accra"),
            &String::from_str(&env, "Cairo"),
            &make_milestones(&env),
            &escrow,
        )
        .unwrap();

    let ids = client.get_shipments_by_shipper(&shipper);
    assert_eq!(ids.len(), 2);
}
