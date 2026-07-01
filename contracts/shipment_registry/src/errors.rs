use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ShipmentNotFound = 4,
    InvalidStatus = 5,
    MilestoneNotFound = 6,
    MilestoneAlreadyConfirmed = 7,
    InvalidLocationData = 8,
    InvalidMilestoneCount = 9,
    ShipmentClosed = 10,
}
