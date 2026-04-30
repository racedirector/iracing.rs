#![warn(missing_docs)]
//! Access to the state of the iRacing simulation on a host PC.
//!
//! # Features
//! - **Sim status**: Access to the sim running status via HTTP.
//! - **Windows process detection**: Detect whether the iRacing executable is running.
//!
//! # Quick Start
//!
//! See examples.

#[cfg(windows)]
mod process;
mod simulation;

#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use process::{
    DEFAULT_IRACING_PROCESS_NAME, ProcessDetectionError, is_iracing_process_running,
};
pub use simulation::{
    DEFAULT_HOST, DEFAULT_PORT, SIM_STATUS_PATH, SimStatusClient, SimStatusError,
    SimStatusResponse, Simulation, StdSimStatusClient, sim_status_url,
};
