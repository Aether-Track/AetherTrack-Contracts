#![no_std]

mod errors;
mod events;
mod types;

use errors::Error;
use soroban_sdk::{
    contract, contractimpl, Address, Bytes, BytesN, Env, String, Vec,
};
use types::{
    CargoDetails, DataKey, LocationProof, Milestone, MilestoneInput, Shipment, ShipmentStatus,
};

fn only_authorized(env: &Env, shipment: &Shipment, caller: &Address) -> Result<(), Error> {
    caller.require_auth();
    if caller != &shipment.shipper
        && caller != &shipment.carrier
        && caller != &shipment.receiver
    {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != &admin {
            return Err(Error::Unauthorized);
        }
    }
    Ok(())
}

fn get_shipment(env: &Env, id: &Bytes) -> Result<Shipment, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Shipment(id.clone()))
        .ok_or(Error::ShipmentNotFound)
}

fn save_shipment(env: &Env, shipment: &Shipment) {
    env.storage()
        .persistent()
        .set(&DataKey::Shipment(shipment.id.clone()), shipment);
    // Extend TTL for 1 year in ledgers (~5s/ledger => ~6_307_200 ledgers)
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Shipment(shipment.id.clone()), 6_307_200, 6_307_200);
}

fn append_to_address_index(env: &Env, key: DataKey, id: &Bytes) {
    let mut list: Vec<Bytes> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    list.push_back(id.clone());
    env.storage().persistent().set(&key, &list);
}

fn generate_id(env: &Env) -> Bytes {
    let count: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ShipmentCount)
        .unwrap_or(0u64);
    env.storage()
        .instance()
        .set(&DataKey::ShipmentCount, &(count + 1));

    // XOR hash of ledger sequence + timestamp + count for uniqueness
    let seed: BytesN<32> = env.crypto().sha256(
        &Bytes::from_array(
            env,
            &{
                let mut buf = [0u8; 32];
                let ts = env.ledger().timestamp().to_be_bytes();
                let seq = env.ledger().sequence().to_be_bytes();
                let cnt = count.to_be_bytes();
                buf[0..8].copy_from_slice(&ts);
                buf[8..12].copy_from_slice(&seq);
                buf[12..20].copy_from_slice(&cnt);
                buf
            },
        ),
    );
    Bytes::from(seed)
}

#[contract]
pub struct ShipmentRegistryContract;

#[contractimpl]
impl ShipmentRegistryContract {
    /// One-time initializer — sets the admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Create a new shipment. Returns the generated shipment ID.
    pub fn create_shipment(
        env: Env,
        shipper: Address,
        receiver: Address,
        carrier: Address,
        cargo: CargoDetails,
        origin: String,
        destination: String,
        milestones: Vec<MilestoneInput>,
        escrow_contract: Address,
    ) -> Result<Bytes, Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        if milestones.len() > 20 {
            return Err(Error::InvalidMilestoneCount);
        }

        shipper.require_auth();

        let id = generate_id(&env);
        let now = env.ledger().timestamp();

        let mut milestone_vec: Vec<Milestone> = Vec::new(&env);
        for m in milestones.iter() {
            milestone_vec.push_back(Milestone {
                description: m.description.clone(),
                location_name: m.location_name.clone(),
                expected_ts: m.expected_ts,
                actual_ts: 0,
                confirmed: false,
                confirmed_by: shipper.clone(), // placeholder; overwritten on confirm
            });
        }

        let shipment = Shipment {
            id: id.clone(),
            shipper: shipper.clone(),
            receiver: receiver.clone(),
            carrier: carrier.clone(),
            cargo,
            status: ShipmentStatus::Created,
            origin,
            destination,
            milestones: milestone_vec,
            location_history: Vec::new(&env),
            created_at: now,
            updated_at: now,
            escrow_contract,
        };

        save_shipment(&env, &shipment);
        append_to_address_index(
            &env,
            DataKey::ShipperIndex(shipper.clone()),
            &id,
        );
        append_to_address_index(
            &env,
            DataKey::CarrierIndex(carrier.clone()),
            &id,
        );
        append_to_address_index(
            &env,
            DataKey::ReceiverIndex(receiver.clone()),
            &id,
        );

        events::shipment_created(&env, &id, &shipper, &carrier);
        Ok(id)
    }

    /// Carrier or any authorized party records a location update.
    pub fn update_location(
        env: Env,
        caller: Address,
        shipment_id: Bytes,
        latitude: i64,
        longitude: i64,
        altitude_m: i32,
        metadata: String,
    ) -> Result<(), Error> {
        let mut shipment = get_shipment(&env, &shipment_id)?;
        only_authorized(&env, &shipment, &caller)?;

        if shipment.status == ShipmentStatus::Delivered
            || shipment.status == ShipmentStatus::Cancelled
        {
            return Err(Error::ShipmentClosed);
        }

        // lat ±90°, lon ±180° (stored * 1_000_000)
        if latitude < -90_000_000 || latitude > 90_000_000 {
            return Err(Error::InvalidLocationData);
        }
        if longitude < -180_000_000 || longitude > 180_000_000 {
            return Err(Error::InvalidLocationData);
        }

        let proof = LocationProof {
            latitude,
            longitude,
            altitude_m,
            timestamp: env.ledger().timestamp(),
            reporter: caller.clone(),
            metadata,
        };

        shipment.location_history.push_back(proof);
        shipment.updated_at = env.ledger().timestamp();

        if shipment.status == ShipmentStatus::Created {
            shipment.status = ShipmentStatus::InTransit;
        }

        save_shipment(&env, &shipment);
        events::location_updated(&env, &shipment_id, &caller, latitude, longitude);
        Ok(())
    }

    /// Confirm a milestone. Caller must be the receiver or admin.
    pub fn confirm_milestone(
        env: Env,
        caller: Address,
        shipment_id: Bytes,
        milestone_index: u32,
    ) -> Result<(), Error> {
        let mut shipment = get_shipment(&env, &shipment_id)?;

        // Only receiver or admin can confirm milestones
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != shipment.receiver && caller != admin {
            return Err(Error::Unauthorized);
        }

        if milestone_index as usize >= shipment.milestones.len() as usize {
            return Err(Error::MilestoneNotFound);
        }

        let mut milestone = shipment.milestones.get(milestone_index).unwrap();
        if milestone.confirmed {
            return Err(Error::MilestoneAlreadyConfirmed);
        }

        milestone.confirmed = true;
        milestone.actual_ts = env.ledger().timestamp();
        milestone.confirmed_by = caller.clone();
        shipment.milestones.set(milestone_index, milestone);
        shipment.updated_at = env.ledger().timestamp();

        save_shipment(&env, &shipment);
        events::milestone_confirmed(&env, &shipment_id, milestone_index, &caller);
        Ok(())
    }

    /// Update the overall shipment status. Shipper or admin only.
    pub fn update_status(
        env: Env,
        caller: Address,
        shipment_id: Bytes,
        new_status: ShipmentStatus,
    ) -> Result<(), Error> {
        let mut shipment = get_shipment(&env, &shipment_id)?;

        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != shipment.shipper && caller != admin {
            return Err(Error::Unauthorized);
        }

        if shipment.status == ShipmentStatus::Delivered
            || shipment.status == ShipmentStatus::Cancelled
        {
            return Err(Error::ShipmentClosed);
        }

        shipment.status = new_status.clone();
        shipment.updated_at = env.ledger().timestamp();

        save_shipment(&env, &shipment);
        events::status_changed(&env, &shipment_id, &caller, &new_status);
        Ok(())
    }

    /// Transfer custody from current carrier to a new one.
    pub fn transfer_custody(
        env: Env,
        caller: Address,
        shipment_id: Bytes,
        new_carrier: Address,
    ) -> Result<(), Error> {
        let mut shipment = get_shipment(&env, &shipment_id)?;

        caller.require_auth();
        if caller != shipment.carrier && caller != shipment.shipper {
            let admin: Address = env
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .ok_or(Error::Unauthorized)?;
            if caller != admin {
                return Err(Error::Unauthorized);
            }
        }

        if shipment.status == ShipmentStatus::Delivered
            || shipment.status == ShipmentStatus::Cancelled
        {
            return Err(Error::ShipmentClosed);
        }

        let old_carrier = shipment.carrier.clone();
        shipment.carrier = new_carrier.clone();
        shipment.updated_at = env.ledger().timestamp();

        // Update carrier index
        append_to_address_index(
            &env,
            DataKey::CarrierIndex(new_carrier.clone()),
            &shipment_id,
        );

        save_shipment(&env, &shipment);
        events::custody_transferred(&env, &shipment_id, &old_carrier, &new_carrier);
        Ok(())
    }

    // ── Read-only queries ────────────────────────────────────────────────────

    pub fn get_shipment(env: Env, shipment_id: Bytes) -> Result<Shipment, Error> {
        get_shipment(&env, &shipment_id)
    }

    pub fn get_shipments_by_shipper(env: Env, shipper: Address) -> Vec<Bytes> {
        env.storage()
            .persistent()
            .get(&DataKey::ShipperIndex(shipper))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_shipments_by_carrier(env: Env, carrier: Address) -> Vec<Bytes> {
        env.storage()
            .persistent()
            .get(&DataKey::CarrierIndex(carrier))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_shipments_by_receiver(env: Env, receiver: Address) -> Vec<Bytes> {
        env.storage()
            .persistent()
            .get(&DataKey::ReceiverIndex(receiver))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_location_history(env: Env, shipment_id: Bytes) -> Result<Vec<LocationProof>, Error> {
        let shipment = get_shipment(&env, &shipment_id)?;
        Ok(shipment.location_history)
    }

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }
}
