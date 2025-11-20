use iracing::simulation::Simulation;

pub fn main() {
    let simulation = Simulation {
        host: String::from("127.0.0.1"),
    };

    println!("Waiting for iRacing simulation connection...");

    // Check for a sim connection every second
    while !simulation.is_connected() {
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    println!("iRacing connected!");

    // Check for sim disconnection every second
    while simulation.is_connected() {
        // !!!: Here is where you'd do simulation related work; start your connection, read session info, etc.
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    println!("iRacing disconnected!")
}
