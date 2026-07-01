use soroban_sdk::{symbol_short, Address, Bytes, Env};

pub fn escrow_funded(env: &Env, shipment_id: &Bytes, payer: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("FUNDED"), shipment_id), (payer, amount));
}

pub fn milestone_released(
    env: &Env,
    shipment_id: &Bytes,
    caller: &Address,
    index: u32,
    amount: i128,
) {
    env.events()
        .publish((symbol_short!("RELEASED"), shipment_id), (caller, index, amount));
}

pub fn dispute_raised(env: &Env, shipment_id: &Bytes, caller: &Address) {
    env.events()
        .publish((symbol_short!("DISPUTE"), shipment_id), caller);
}

pub fn dispute_resolved(env: &Env, shipment_id: &Bytes, resolver: &Address, to_payee: bool) {
    env.events()
        .publish((symbol_short!("RESOLVED"), shipment_id), (resolver, to_payee));
}
