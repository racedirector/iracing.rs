//! Checks to see if the iRacing simulation is running on the local host.
//! Will print `running=true` if the simulation is running, else `running=false`.
//!
use iracing_simulation::Simulation;

fn main() {
    let sim = Simulation::new("127.0.0.1", 32034);
    println!("running={}", sim.check_sim_status());
}
