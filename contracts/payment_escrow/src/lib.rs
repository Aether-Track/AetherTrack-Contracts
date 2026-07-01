#![no_std]

mod errors;
mod events;
mod types;

use errors::Error;
use soroban_sdk::{contract, contractimpl, token, Address, Bytes, Env, Vec};
use types::{DataKey, Escrow, EscrowStatus, MilestonePayment};

fn load_escrow(env: &Env) -> Result<Escrow, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Escrow)
        .ok_or(Error::NotInitialized)
}

fn save_escrow(env: &Env, escrow: &Escrow) {
    env.storage().instance().set(&DataKey::Escrow, escrow);
}

#[contract]
pub struct PaymentEscrowContract;

#[contractimpl]
impl PaymentEscrowContract {
    /// Initialize escrow. Called once by the shipper (payer) when creating a shipment.
    /// milestone_amounts must sum to total_amount.
    pub fn initialize(
        env: Env,
        shipment_id: Bytes,
        payer: Address,
        payee: Address,
        token: Address,
        total_amount: i128,
        milestone_amounts: Vec<i128>,
        dispute_resolver: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Escrow) {
            return Err(Error::AlreadyInitialized);
        }
        if total_amount <= 0 {
            return Err(Error::ZeroAmount);
        }

        // Validate milestone amounts sum to total
        let mut sum: i128 = 0;
        let mut payments: Vec<MilestonePayment> = Vec::new(&env);
        for amount in milestone_amounts.iter() {
            if amount <= 0 {
                return Err(Error::ZeroAmount);
            }
            sum += amount;
            payments.push_back(MilestonePayment {
                amount_stroops: amount,
                released: false,
                released_at: 0,
            });
        }
        if sum != total_amount {
            return Err(Error::InvalidMilestoneAmounts);
        }

        payer.require_auth();

        let now = env.ledger().timestamp();
        let escrow = Escrow {
            shipment_id,
            payer,
            payee,
            token,
            total_amount,
            deposited_amount: 0,
            milestone_payments: payments,
            status: EscrowStatus::Pending,
            dispute_resolver,
            created_at: now,
            updated_at: now,
        };

        save_escrow(&env, &escrow);
        Ok(())
    }

    /// Payer deposits tokens into the escrow contract.
    pub fn deposit(env: Env, amount: i128) -> Result<(), Error> {
        let mut escrow = load_escrow(&env)?;

        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Active {
            return Err(Error::EscrowNotActive);
        }
        if amount <= 0 {
            return Err(Error::ZeroAmount);
        }
        if escrow.deposited_amount >= escrow.total_amount {
            return Err(Error::AlreadyFullyFunded);
        }

        escrow.payer.require_auth();

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &escrow.payer,
            &env.current_contract_address(),
            &amount,
        );

        escrow.deposited_amount += amount;
        escrow.status = EscrowStatus::Active;
        escrow.updated_at = env.ledger().timestamp();

        events::escrow_funded(&env, &escrow.shipment_id, &escrow.payer, amount);
        save_escrow(&env, &escrow);
        Ok(())
    }

    /// Release payment for a confirmed milestone. Called by payer or dispute_resolver.
    pub fn release_milestone(env: Env, caller: Address, milestone_index: u32) -> Result<(), Error> {
        let mut escrow = load_escrow(&env)?;

        caller.require_auth();
        if caller != escrow.payer && caller != escrow.dispute_resolver {
            return Err(Error::Unauthorized);
        }
        if escrow.status != EscrowStatus::Active
            && escrow.status != EscrowStatus::PartiallyReleased
        {
            return Err(Error::EscrowNotActive);
        }

        if milestone_index as usize >= escrow.milestone_payments.len() as usize {
            return Err(Error::MilestoneNotFound);
        }

        let mut payment = escrow.milestone_payments.get(milestone_index).unwrap();
        if payment.released {
            return Err(Error::MilestoneAlreadyReleased);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.payee,
            &payment.amount_stroops,
        );

        payment.released = true;
        payment.released_at = env.ledger().timestamp();
        escrow.milestone_payments.set(milestone_index, payment.clone());
        escrow.updated_at = env.ledger().timestamp();

        // Check if all milestones are now released
        let all_released = escrow
            .milestone_payments
            .iter()
            .all(|p| p.released);

        escrow.status = if all_released {
            EscrowStatus::FullyReleased
        } else {
            EscrowStatus::PartiallyReleased
        };

        events::milestone_released(
            &env,
            &escrow.shipment_id,
            &caller,
            milestone_index,
            payment.amount_stroops,
        );
        save_escrow(&env, &escrow);
        Ok(())
    }

    /// Release all remaining milestone payments at once (e.g. on successful delivery).
    pub fn release_all(env: Env, caller: Address) -> Result<(), Error> {
        let mut escrow = load_escrow(&env)?;

        caller.require_auth();
        if caller != escrow.payer && caller != escrow.dispute_resolver {
            return Err(Error::Unauthorized);
        }
        if escrow.status != EscrowStatus::Active
            && escrow.status != EscrowStatus::PartiallyReleased
        {
            return Err(Error::EscrowNotActive);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        let now = env.ledger().timestamp();

        for i in 0..escrow.milestone_payments.len() {
            let mut payment = escrow.milestone_payments.get(i).unwrap();
            if !payment.released {
                token_client.transfer(
                    &env.current_contract_address(),
                    &escrow.payee,
                    &payment.amount_stroops,
                );
                payment.released = true;
                payment.released_at = now;
                escrow.milestone_payments.set(i, payment);
            }
        }

        escrow.status = EscrowStatus::FullyReleased;
        escrow.updated_at = now;
        save_escrow(&env, &escrow);
        Ok(())
    }

    /// Raise a dispute — freezes the escrow.
    pub fn raise_dispute(env: Env, caller: Address) -> Result<(), Error> {
        let mut escrow = load_escrow(&env)?;

        caller.require_auth();
        if caller != escrow.payer && caller != escrow.payee {
            return Err(Error::Unauthorized);
        }
        if escrow.status != EscrowStatus::Active
            && escrow.status != EscrowStatus::PartiallyReleased
        {
            return Err(Error::EscrowNotActive);
        }

        escrow.status = EscrowStatus::Disputed;
        escrow.updated_at = env.ledger().timestamp();

        events::dispute_raised(&env, &escrow.shipment_id, &caller);
        save_escrow(&env, &escrow);
        Ok(())
    }

    /// Dispute resolver settles the dispute by sending remaining funds to payer or payee.
    pub fn resolve_dispute(
        env: Env,
        resolver: Address,
        release_to_payee: bool,
    ) -> Result<(), Error> {
        let mut escrow = load_escrow(&env)?;

        resolver.require_auth();
        if resolver != escrow.dispute_resolver {
            return Err(Error::Unauthorized);
        }
        if escrow.status != EscrowStatus::Disputed {
            return Err(Error::DisputeNotActive);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        let now = env.ledger().timestamp();
        let recipient = if release_to_payee {
            escrow.payee.clone()
        } else {
            escrow.payer.clone()
        };

        // Transfer all unreleased funds to the determined recipient
        for i in 0..escrow.milestone_payments.len() {
            let mut payment = escrow.milestone_payments.get(i).unwrap();
            if !payment.released {
                token_client.transfer(
                    &env.current_contract_address(),
                    &recipient,
                    &payment.amount_stroops,
                );
                payment.released = true;
                payment.released_at = now;
                escrow.milestone_payments.set(i, payment);
            }
        }

        escrow.status = if release_to_payee {
            EscrowStatus::FullyReleased
        } else {
            EscrowStatus::Refunded
        };
        escrow.updated_at = now;

        events::dispute_resolved(&env, &escrow.shipment_id, &resolver, release_to_payee);
        save_escrow(&env, &escrow);
        Ok(())
    }

    // ── Read-only ────────────────────────────────────────────────────────────

    pub fn get_escrow(env: Env) -> Result<Escrow, Error> {
        load_escrow(&env)
    }

    pub fn get_balance(env: Env) -> Result<i128, Error> {
        let escrow = load_escrow(&env)?;
        let token_client = token::Client::new(&env, &escrow.token);
        Ok(token_client.balance(&env.current_contract_address()))
    }
}
