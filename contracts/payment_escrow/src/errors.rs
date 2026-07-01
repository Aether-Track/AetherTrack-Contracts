use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InsufficientDeposit = 4,
    AlreadyFullyFunded = 5,
    MilestoneAlreadyReleased = 6,
    MilestoneNotFound = 7,
    InvalidMilestoneAmounts = 8,
    EscrowNotActive = 9,
    DisputeNotActive = 10,
    ZeroAmount = 11,
}
