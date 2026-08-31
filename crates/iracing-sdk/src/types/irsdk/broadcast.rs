//! SDK value types used by the broadcast-message protocol.

use super::macros::sdk_enum;

sdk_enum! {
    /// `irsdk_BroadcastMsg`.
    pub enum BroadcastMessage {
        CameraSwitchPosition = 0,
        CameraSwitchNumber = 1,
        CameraSetState = 2,
        ReplaySetPlaySpeed = 3,
        ReplaySetPlayPosition = 4,
        ReplaySearch = 5,
        ReplaySetState = 6,
        ReloadTextures = 7,
        ChatCommand = 8,
        PitCommand = 9,
        TelemetryCommand = 10,
        ForceFeedbackCommand = 11,
        ReplaySearchSessionTime = 12,
        VideoCapture = 13,
        Last = 14,
    }
}

sdk_enum! {
    /// `irsdk_ChatCommandMode`.
    pub enum ChatCommandMode {
        Macro = 0,
        BeginChat = 1,
        Reply = 2,
        Cancel = 3,
    }
}

sdk_enum! {
    /// `irsdk_PitCommandMode`.
    pub enum PitCommandMode {
        Clear = 0,
        WindshieldTearoff = 1,
        Fuel = 2,
        LeftFrontTire = 3,
        RightFrontTire = 4,
        LeftRearTire = 5,
        RightRearTire = 6,
        ClearTires = 7,
        FastRepair = 8,
        ClearWindshieldTearoff = 9,
        ClearFastRepair = 10,
        ClearFuel = 11,
        TireCompound = 12,
    }
}

sdk_enum! {
    /// `irsdk_TelemCommandMode`.
    pub enum TelemetryCommandMode {
        Stop = 0,
        Start = 1,
        Restart = 2,
    }
}

sdk_enum! {
    /// `irsdk_RpyStateMode`.
    pub enum ReplayStateMode {
        EraseTape = 0,
        Last = 1,
    }
}

sdk_enum! {
    /// `irsdk_ReloadTexturesMode`.
    pub enum ReloadTexturesMode {
        All = 0,
        CarIndex = 1,
    }
}

sdk_enum! {
    /// `irsdk_RpySrchMode`.
    pub enum ReplaySearchMode {
        ToStart = 0,
        ToEnd = 1,
        PrevSession = 2,
        NextSession = 3,
        PrevLap = 4,
        NextLap = 5,
        PrevFrame = 6,
        NextFrame = 7,
        PrevIncident = 8,
        NextIncident = 9,
        Last = 10,
    }
}

sdk_enum! {
    /// `irsdk_RpyPosMode`.
    pub enum ReplayPositionMode {
        Begin = 0,
        Current = 1,
        End = 2,
        Last = 3,
    }
}

sdk_enum! {
    /// `irsdk_FFBCommandMode`.
    pub enum ForceFeedbackCommandMode {
        MaxForce = 0,
        Last = 1,
    }
}

sdk_enum! {
    /// `irsdk_csMode`. Negative values select special camera focus targets.
    pub enum CameraSwitchFocusMode {
        FocusAtIncident = -3,
        FocusAtLeader = -2,
        FocusAtExiting = -1,
        FocusAtDriver = 0,
    }
}

sdk_enum! {
    /// `irsdk_VideoCaptureMode`.
    pub enum VideoCaptureMode {
        TriggerScreenshot = 0,
        StartVideoCapture = 1,
        EndVideoCapture = 2,
        ToggleVideoCapture = 3,
        ShowVideoTimer = 4,
        HideVideoTimer = 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinels_and_signed_focus_values_match_the_sdk() {
        assert_eq!(i32::from(BroadcastMessage::Last), 14);
        assert_eq!(i32::from(ReplaySearchMode::Last), 10);
        assert_eq!(i32::from(CameraSwitchFocusMode::FocusAtIncident), -3);
        assert_eq!(CameraSwitchFocusMode::try_from(-4), Err(-4));
    }
}
