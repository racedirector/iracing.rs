use clap::builder::{PossibleValuesParser, TypedValueParser};
use iracing_broadcast_sdk::{
    CameraState, CameraSwitchFocusMode, ReplayPositionMode, ReplaySearchMode, TelemetryCommandMode,
    VideoCaptureMode,
};

macro_rules! possible_value_parser {
    (
      // The name of the function to generate
      $name:ident,
      // The type that we're parsing to
      $type:ty,
      // The possible values
      $(($value:literal, ::$variant:ident)),+
    ) => {
      pub fn $name() -> impl TypedValueParser<Value = $type> {
        PossibleValuesParser::new([$($value),+])
        .map(|value| match value.as_str() {
          $(
            $value => <$type>::$variant,
          )+
          _ => unreachable!("validated by PossibleValuesParser")
        })
      }
    };
}

possible_value_parser!(
    replay_position_parser,
    ReplayPositionMode,
    ("begin", ::Begin),
    ("current", ::Current),
    ("end", ::End)
);

possible_value_parser!(
    replay_search_parser,
    ReplaySearchMode,
    ("to-start", ::ToStart),
    ("to-end", ::ToEnd),
    ("next-session", ::NextSession),
    ("previous-session", ::PreviousSession),
    ("next-lap", ::NextLap),
    ("previous-lap", ::PreviousLap),
    ("next-frame", ::NextFrame),
    ("previous-frame", ::PreviousFrame),
    ("next-incident", ::NextIncident),
    ("previous-incident", ::PreviousIncident)
);

possible_value_parser!(
    telemetry_command_parser,
    TelemetryCommandMode,
    ("stop", ::Stop),
    ("start", ::Start),
    ("restart", ::Restart)
);

possible_value_parser!(
    video_command_parser,
    VideoCaptureMode,
    ("screenshot", ::TriggerScreenshot),
    ("start", ::StartVideoCapture),
    ("stop", ::EndVideoCapture),
    ("toggle", ::ToggleVideoCapture),
    ("show-timer", ::ShowVideoTimer),
    ("hide-timer", ::HideVideoTimer)
);

possible_value_parser!(
    camera_state_parser,
    CameraState,
    ("cam-tool-active", ::CAMERA_TOOL_ACTIVE),
    ("ui-hidden", ::USER_INTERFACE_HIDDEN),
    ("auto-shot-selection", ::USE_AUTO_SHOT_SELECTION),
    ("temporary-edits", ::USE_TEMPORARY_EDITS),
    ("key-acceleration", ::USE_KEY_ACCELERATION),
    ("10x-acceleration", ::USE_KEY_TEN_TIMES_ACCELERATION),
    ("mouse-aim", ::USE_MOUSE_AIM_MODE)
);

// possible_value_parser!(
//     camera_switch_focus_parser,
//     CameraSwitchFocusMode,
//     ("incident", ::FocusAtIncident),
//     ("leader", ::FocusAtLeader),
//     ("exiting", ::FocusAtExiting),
//     ("driver", ::FocusAtDriver)
// );
