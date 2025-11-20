use iracing::{
    broadcast::{Broadcast, BroadcastMessage, PitCommandMode},
    simulation::Simulation,
};

pub fn main() {
    let simulation = Simulation {
        host: String::from("127.0.0.1"),
    };

    while !simulation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    println!("iRacing connected, attempting to reload all textures...");

    let broadcast = Broadcast::new();

    broadcast.send_message(BroadcastMessage::ReloadAllTextures);

    // 4-tire change with pressure-adjustment
    broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::LF(176)));
    broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::RF(176)));
    broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::LR(176)));
    broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::RR(176)));

    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::ClearTires));

    // // 4-tire change with NO pressure-adjustment
    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::LF(0)));
    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::RF(0)));
    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::LR(0)));
    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::RR(0)));

    // broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::ClearTires));
}
