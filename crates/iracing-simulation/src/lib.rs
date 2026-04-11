#![warn(missing_docs)]
//! Access to the state of the iRacing simulation on a host PC.
//!
//! # Features
//! - **Sim status**: Access to the sim running status via HTTP.
//!
//! # Quick Start
//!
//! See examples.

mod simulation;

pub use simulation::{
    DEFAULT_HOST, DEFAULT_PORT, SIM_STATUS_PATH, SimStatusClient, SimStatusError,
    SimStatusResponse, Simulation, StdSimStatusClient, sim_status_url,
};
