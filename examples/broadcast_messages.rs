use iracing::{broadcast::Broadcast, simulation::Simulation};

pub fn main() {
    let simuation = Simulation {
        host: "127.0.0.1".to_string(),
    };

    while !simuation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    let broadcast = Broadcast::new();

    broadcast.reload_all_textures();
    broadcast.reload_textures(0);
}
