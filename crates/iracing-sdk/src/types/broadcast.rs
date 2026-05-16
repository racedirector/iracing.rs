//! Typed iRacing broadcast command helpers.

/// Commands that adjust pit service behavior for the player's car.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitCommand {
    /// Clear all pending pit service selections.
    Clear,
    /// Request a windshield tearoff.
    Tearoff,
    /// Set fuel amount in gallons.
    Fuel(u16),
    /// Set left-front tire pressure in PSI.
    LF(u16),
    /// Set right-front tire pressure in PSI.
    RF(u16),
    /// Set left-rear tire pressure in PSI.
    LR(u16),
    /// Set right-rear tire pressure in PSI.
    RR(u16),
    /// Clear all tire pressure changes.
    ClearTires,
    /// Request fast repair.
    FastRepair,
    /// Clear windshield tearoff request.
    ClearTearoff,
    /// Clear fast repair request.
    ClearFastRepair,
    /// Clear fuel request.
    ClearFuel,
}
