#![warn(missing_docs)]
//! Independent probes for the iRacing simulation on a host PC.
//!
//! [`Simulation`] checks the HTTP status endpoint through a synchronous
//! [`SimStatusClient`]. [`StdSimStatusClient`] provides the built-in TCP transport;
//! applications can inject another client without introducing an HTTP runtime
//! into the library. Construction stores configuration and performs no I/O.
//!
//! Windows builds also expose `is_iracing_process_running`, which checks
//! executable presence independently of HTTP status. Process presence does not
//! establish endpoint readiness or telemetry connectivity. Neither probe starts,
//! stops, retries, or monitors the simulator; callers own lifecycle policy.
//!
//! # Usage
//!
//! ```no_run
//! use iracing_simulation::Simulation;
//!
//! let simulation = Simulation::local();
//! let running = simulation.check_sim_status();
//! ```
//!
//! The crate's `examples/` directory demonstrates injected clients and polling.
//! Method documentation owns response semantics; the implementation's unit tests
//! exercise status handling and Windows executable-name matching.

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
