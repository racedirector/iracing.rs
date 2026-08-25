//! SDK value types used by telemetry variables.

use super::macros::sdk_enum;

sdk_enum! {
    /// `irsdk_TrkLoc`.
    pub enum TrackLocation {
        NotInWorld = -1,
        OffTrack = 0,
        InPitStall = 1,
        ApproachingPits = 2,
        OnTrack = 3,
    }
}

sdk_enum! {
    /// `irsdk_TrkSurf`.
    pub enum TrackSurface {
        SurfaceNotInWorld = -1,
        UndefinedMaterial = 0,
        Asphalt1Material = 1,
        Asphalt2Material = 2,
        Asphalt3Material = 3,
        Asphalt4Material = 4,
        Concrete1Material = 5,
        Concrete2Material = 6,
        RacingDirt1Material = 7,
        RacingDirt2Material = 8,
        Paint1Material = 9,
        Paint2Material = 10,
        Rumble1Material = 11,
        Rumble2Material = 12,
        Rumble3Material = 13,
        Rumble4Material = 14,
        Grass1Material = 15,
        Grass2Material = 16,
        Grass3Material = 17,
        Grass4Material = 18,
        Dirt1Material = 19,
        Dirt2Material = 20,
        Dirt3Material = 21,
        Dirt4Material = 22,
        SandMaterial = 23,
        Gravel1Material = 24,
        Gravel2Material = 25,
        GrasscreteMaterial = 26,
        AstroturfMaterial = 27,
    }
}

sdk_enum! {
    /// `irsdk_SessionState`.
    pub enum SessionState {
        Invalid = 0,
        GetInCar = 1,
        Warmup = 2,
        ParadeLaps = 3,
        Racing = 4,
        Checkered = 5,
        CoolDown = 6,
    }
}

sdk_enum! {
    /// `irsdk_CarLeftRight`.
    pub enum CarLeftRight {
        Off = 0,
        Clear = 1,
        CarLeft = 2,
        CarRight = 3,
        CarLeftRight = 4,
        TwoCarsLeft = 5,
        TwoCarsRight = 6,
    }
}

sdk_enum! {
    /// `irsdk_PitSvStatus`.
    pub enum PitServiceStatus {
        None = 0,
        InProgress = 1,
        Complete = 2,
        TooFarLeft = 100,
        TooFarRight = 101,
        TooFarForward = 102,
        TooFarBack = 103,
        BadAngle = 104,
        CantFixThat = 105,
    }
}

sdk_enum! {
    /// `irsdk_PaceMode`.
    pub enum PaceMode {
        SingleFileStart = 0,
        DoubleFileStart = 1,
        SingleFileRestart = 2,
        DoubleFileRestart = 3,
        NotPacing = 4,
    }
}

sdk_enum! {
    /// `irsdk_TrackWetness`.
    pub enum TrackWetness {
        Unknown = 0,
        Dry = 1,
        MostlyDry = 2,
        VeryLightlyWet = 3,
        LightlyWet = 4,
        ModeratelyWet = 5,
        VeryWet = 6,
        ExtremelyWet = 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_values_are_preserved() {
        assert_eq!(i32::from(TrackLocation::NotInWorld), -1);
        assert_eq!(i32::from(TrackSurface::AstroturfMaterial), 27);
        assert_eq!(i32::from(PitServiceStatus::None), 0);
        assert_eq!(i32::from(PitServiceStatus::TooFarLeft), 100);
        assert_eq!(TrackWetness::try_from(8), Err(8));
    }
}
