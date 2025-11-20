use iracing::simulation::Simulation;

pub fn main() {
    let simulation = Simulation {
        host: "127.0.0.1".to_string(),
    };

    println!("Waiting for iRacing simulation connection...");

    while !simulation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    println!("iRacing connected!");

    while simulation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    println!("iRacing disconnected!")
}
