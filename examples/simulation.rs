use iracing::simulation::Simulation;
use std::{thread::sleep, time::Duration};

pub fn main() {
    let simulation = Simulation {
        host: String::from("127.0.0.1"),
    };

    loop {
        println!("Waiting for iRacing simulation connection...");
        // Wait for a connection
        while !simulation.is_connected() {
            sleep(Duration::from_secs(1))
        }

        println!("iRacing connected!");

        while simulation.is_connected() {
            sleep(Duration::from_millis(500))
        }

        println!("iRacing disconnected!");
    }
}
