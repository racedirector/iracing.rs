use iracing::{
    broadcast::{
        BroadcastMessage, ChatCommandMode, Client, PitCommandMode, ReplayPositionMode,
        ReplaySearchMode, TelemetryCommandMode, VideoCaptureMode,
    },
    simulation::Simulation,
    states::CameraState,
};

pub fn main() {
    let simulation = Simulation::local();

    while !simulation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    let broadcast = Client::new().expect("Could not create broadcast client");

    demo_camera_messages(&broadcast);
    demo_replay_messages(&broadcast);
    demo_chat_messages(&broadcast);
    demo_pit_messages(&broadcast);
    demo_telemetry_and_ffb(&broadcast);
    demo_video_capture(&broadcast);
}

fn demo_camera_messages(broadcast: &Client) {
    broadcast.send_message(BroadcastMessage::CameraSwitchPosition(0, 0, 0));
    broadcast.send_message(BroadcastMessage::CameraSwitchNumber("064", 1, 1));
    let scenic_camera = CameraState::IS_SCENIC_ACTIVE | CameraState::UI_HIDDEN;
    broadcast.send_message(BroadcastMessage::CameraSetState(scenic_camera));
}

fn demo_replay_messages(broadcast: &Client) {
    broadcast.send_message(BroadcastMessage::ReplaySetPlaySpeed(1, false));
    broadcast.send_message(BroadcastMessage::ReplaySetPlaySpeed(4, true));
    broadcast.send_message(BroadcastMessage::ReplaySetPlayPosition(
        ReplayPositionMode::Begin,
        0,
    ));
    broadcast.send_message(BroadcastMessage::ReplaySearch(
        ReplaySearchMode::NextIncident,
    ));
    broadcast.send_message(BroadcastMessage::ReplaySetState);
    broadcast.send_message(BroadcastMessage::ReplaySearchSessionTime(0, 15_000));
    broadcast.send_message(BroadcastMessage::ReloadAllTextures);
    broadcast.send_message(BroadcastMessage::ReloadTextures(12));
}

fn demo_chat_messages(broadcast: &Client) {
    for mode in [
        ChatCommandMode::Macro,
        ChatCommandMode::Begin,
        ChatCommandMode::Reply,
        ChatCommandMode::Cancel,
    ] {
        broadcast.send_message(BroadcastMessage::ChatCommand(mode));
    }
    broadcast.send_message(BroadcastMessage::ChatCommandMacro(3));
}

fn demo_pit_messages(broadcast: &Client) {
    let pit_modes = [
        PitCommandMode::Clear,
        PitCommandMode::Tearoff,
        PitCommandMode::Fuel(65),
        PitCommandMode::LF(26),
        PitCommandMode::RF(26),
        PitCommandMode::LR(26),
        PitCommandMode::RR(26),
        PitCommandMode::ClearTires,
        PitCommandMode::FastRepair,
        PitCommandMode::ClearTearoff,
        PitCommandMode::ClearFastRepair,
        PitCommandMode::ClearFuel,
    ];

    for mode in pit_modes {
        broadcast.send_message(BroadcastMessage::PitCommand(mode));
    }
}

fn demo_telemetry_and_ffb(broadcast: &Client) {
    broadcast.send_message(BroadcastMessage::TelemetryCommand(
        TelemetryCommandMode::Restart,
    ));
    broadcast.send_message(BroadcastMessage::FFBCommand(32_768));
}

fn demo_video_capture(broadcast: &Client) {
    broadcast.send_message(BroadcastMessage::VideoCapture(
        VideoCaptureMode::ToggleCapture,
    ));
}
