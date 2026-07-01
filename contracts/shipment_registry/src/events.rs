use soroban_sdk::{symbol_short, Address, Bytes, Env};

use crate::types::ShipmentStatus;

pub fn shipment_created(env: &Env, id: &Bytes, shipper: &Address, carrier: &Address) {
    env.events()
        .publish((symbol_short!("CREATED"), id), (shipper, carrier));
}

pub fn location_updated(env: &Env, id: &Bytes, reporter: &Address, lat: i64, lon: i64) {
    env.events()
        .publish((symbol_short!("LOCATION"), id), (reporter, lat, lon));
}

pub fn milestone_confirmed(env: &Env, id: &Bytes, index: u32, confirmed_by: &Address) {
    env.events()
        .publish((symbol_short!("MILESTONE"), id), (index, confirmed_by));
}

pub fn status_changed(env: &Env, id: &Bytes, caller: &Address, status: &ShipmentStatus) {
    env.events()
        .publish((symbol_short!("STATUS"), id), (caller, status));
}

pub fn custody_transferred(env: &Env, id: &Bytes, from: &Address, to: &Address) {
    env.events()
        .publish((symbol_short!("CUSTODY"), id), (from, to));
}
