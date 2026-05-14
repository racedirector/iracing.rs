//! Owned connection-state observation for the iRacing status UI.
//!
//! This module is written as example-friendly Tauri code. It shows how a Rust
//! backend can own a long-lived lifecycle monitor and push small state changes
//! to React without making the frontend poll.
//!
//! The important design choice is that the monitor is stateful. It does not
//! recompute the whole lifecycle snapshot from scratch on every tick:
//!
//! 1. While waiting for the iRacing process, it checks only the process.
//! 2. After the process has been seen, it checks only the sim-status endpoint.
//! 3. After sim status is ready, it attempts to establish live telemetry.
//! 4. Once telemetry connects, the monitor owns that [`WindowsConnection`] and
//!    checks only that connection's `is_connected()` flag.
//! 5. Each failed check steps back one lifecycle phase. If telemetry
//!    disconnects, the monitor checks sim status once; when sim status is also
//!    unavailable, the next phase is process checking.
//!
//! That shape is useful when the backend will later emit telemetry-derived
//! updates. The UI should not repeatedly create short-lived telemetry
//! connections; the Rust monitor owns the connection and can reuse it for future
//! subscribers or additional event streams.

use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};

#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use iracing_simulation::{is_iracing_process_running, Simulation};

/// Event emitted when the observed iRacing connection snapshot changes.
///
/// The React frontend listens for this exact name with
/// `@tauri-apps/api/event.listen`. The payload is an
/// [`IRacingConnectionState`] serialized as JSON:
///
/// ```json
/// {
///   "process": "connected",
///   "sim": "checking",
///   "telemetry": "disconnected"
/// }
/// ```
///
/// The URI-like prefix is only a naming convention. It keeps app-specific
/// events distinct from generic names such as `"status-changed"`.
pub const CONNECTION_STATE_CHANGED_EVENT: &str = "iracing://connection-state-changed";

/// How often the background monitor advances its lifecycle phase.
///
/// Process startup, sim status, and shared-memory availability change on
/// human-visible timescales, so a one-second interval keeps the UI responsive
/// without producing unnecessary IPC traffic.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Timeout for the local iRacing sim-status HTTP probe.
///
/// `Simulation::local()` defaults to a longer timeout that is reasonable for a
/// command-line probe. This UI monitor checks repeatedly, so a shorter timeout
/// avoids making the backend feel stuck when the endpoint is absent.
#[cfg(windows)]
const SIM_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

/// Shared Tauri state for the connection monitor.
///
/// `ConnectionStateObserver` has two jobs:
///
/// - Guard monitor startup with [`AtomicBool`] so the app starts only one
///   backend monitor thread.
/// - Store the latest public snapshot in a mutex so any new frontend subscriber
///   can get the current state immediately without forcing a new telemetry
///   connection.
///
/// The singleton guard matters in development because React's `StrictMode`
/// intentionally mounts effects twice. Without the guard, a frontend remount
/// could start duplicate Rust monitor threads.
#[derive(Debug)]
pub struct ConnectionStateObserver {
    started: AtomicBool,
    state: Arc<Mutex<IRacingConnectionState>>,
}

impl Default for ConnectionStateObserver {
    fn default() -> Self {
        Self {
            started: AtomicBool::new(false),
            state: Arc::new(Mutex::new(IRacingConnectionState::waiting_for_process())),
        }
    }
}

impl ConnectionStateObserver {
    /// Return the last snapshot published by the background monitor.
    fn current_state(&self) -> IRacingConnectionState {
        *lock_state(&self.state)
    }

    /// Clone the shared snapshot handle for use by the monitor thread.
    fn state_handle(&self) -> Arc<Mutex<IRacingConnectionState>> {
        Arc::clone(&self.state)
    }
}

/// UI-friendly connection status for one lifecycle checkpoint.
///
/// The enum serializes with lower-case values so the TypeScript type can stay
/// simple:
///
/// ```ts
/// type ConnectionStatus = "disconnected" | "checking" | "connected";
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    /// The checkpoint is known to be unavailable.
    Disconnected,

    /// A prerequisite is present, but this checkpoint is not ready yet.
    ///
    /// For example, the iRacing process may be running while the sim-status
    /// endpoint has not started returning `running:1`.
    Checking,

    /// The checkpoint is available.
    Connected,
}

/// Complete lifecycle snapshot consumed by the status-light UI.
///
/// The fields are ordered to match the state machine in
/// `examples/iracing-lifecycle-monitor`:
///
/// 1. Detect the iRacing process.
/// 2. Wait for the local sim-status endpoint to report running.
/// 3. Connect to the live telemetry shared-memory interface.
///
/// The struct serializes to camelCase to keep the command payload idiomatic for
/// JavaScript while preserving Rust-style field names internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IRacingConnectionState {
    /// Status of the operating-system process check.
    pub process: ConnectionStatus,

    /// Status of the local sim-status HTTP check.
    pub sim: ConnectionStatus,

    /// Status of the Windows live-telemetry shared-memory connection.
    pub telemetry: ConnectionStatus,
}

impl IRacingConnectionState {
    /// Construct the all-off state used when iRacing is absent or unsupported.
    fn disconnected() -> Self {
        Self {
            process: ConnectionStatus::Disconnected,
            sim: ConnectionStatus::Disconnected,
            telemetry: ConnectionStatus::Disconnected,
        }
    }

    /// Construct the state used while the monitor is checking process presence.
    fn waiting_for_process() -> Self {
        Self {
            process: ConnectionStatus::Checking,
            sim: ConnectionStatus::Disconnected,
            telemetry: ConnectionStatus::Disconnected,
        }
    }

    /// Construct the state used after the process has been seen.
    fn waiting_for_sim() -> Self {
        Self {
            process: ConnectionStatus::Connected,
            sim: ConnectionStatus::Checking,
            telemetry: ConnectionStatus::Disconnected,
        }
    }

    /// Construct the state used after sim status has reported running.
    fn waiting_for_telemetry() -> Self {
        Self {
            process: ConnectionStatus::Connected,
            sim: ConnectionStatus::Connected,
            telemetry: ConnectionStatus::Checking,
        }
    }

    /// Construct the state used while the owned telemetry connection is active.
    fn connected() -> Self {
        Self {
            process: ConnectionStatus::Connected,
            sim: ConnectionStatus::Connected,
            telemetry: ConnectionStatus::Connected,
        }
    }
}

/// Return the latest owned connection-state snapshot to the frontend.
///
/// This command does not probe iRacing. It simply returns the last state
/// published by the monitor thread. That distinction is important: callers can
/// ask for current state without accidentally creating a second telemetry
/// connection.
#[tauri::command]
pub fn get_connection_state(
    observer: State<'_, ConnectionStateObserver>,
) -> IRacingConnectionState {
    observer.current_state()
}

/// Start the backend monitor and return the current snapshot.
///
/// The frontend should register its event listener before invoking this command:
///
/// 1. Listen for [`CONNECTION_STATE_CHANGED_EVENT`].
/// 2. Invoke `observe_connection_state`.
/// 3. Render the returned snapshot immediately.
/// 4. Apply later event payloads as they arrive.
///
/// The command is idempotent for the process lifetime. Every caller receives
/// the latest shared snapshot, but only the first caller starts the background
/// monitor. New subscribers therefore reuse the same Rust-owned lifecycle state
/// and, once connected, the same Rust-owned telemetry connection.
#[tauri::command]
pub fn observe_connection_state(
    app: AppHandle,
    observer: State<'_, ConnectionStateObserver>,
) -> IRacingConnectionState {
    let current_state = observer.current_state();

    if observer
        .started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let state = observer.state_handle();
        let _ = thread::spawn(move || monitor_connection_state(app, state));
    }

    current_state
}

/// Run the owned lifecycle monitor.
///
/// The Windows implementation owns the real state machine and telemetry
/// connection. The non-Windows implementation publishes a stable disconnected
/// state so the example remains buildable and understandable on every platform.
fn monitor_connection_state(app: AppHandle, state: Arc<Mutex<IRacingConnectionState>>) {
    #[cfg(windows)]
    monitor_windows_connection_state(app, state);

    #[cfg(not(windows))]
    {
        publish_state(&app, &state, IRacingConnectionState::disconnected());
        loop {
            thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Store and emit `next_state` when it differs from the current snapshot.
///
/// The monitor uses this helper for every transition. It keeps the mutex lock
/// short and emits after the lock is released so frontend event delivery cannot
/// block other readers of the shared state.
fn publish_state(
    app: &AppHandle,
    state: &Arc<Mutex<IRacingConnectionState>>,
    next_state: IRacingConnectionState,
) {
    let changed = {
        let mut current_state = lock_state(state);
        if *current_state == next_state {
            false
        } else {
            *current_state = next_state;
            true
        }
    };

    if changed {
        let _ = app.emit(CONNECTION_STATE_CHANGED_EVENT, next_state);
    }
}

/// Lock the shared snapshot and recover from poisoning.
///
/// This example has no fallible recovery path that would be useful to expose to
/// the UI. If a thread panics while holding the mutex, we keep the last value
/// and continue using the recovered guard.
fn lock_state(
    state: &Arc<Mutex<IRacingConnectionState>>,
) -> MutexGuard<'_, IRacingConnectionState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Internal Windows lifecycle phase.
///
/// This enum is deliberately not serialized. It describes what the monitor
/// should check next, while [`IRacingConnectionState`] describes what the UI
/// should render. Keeping those separate lets the backend own richer state, such
/// as the active telemetry connection, without leaking implementation details
/// into the frontend contract.
#[cfg(windows)]
enum MonitorPhase {
    /// The monitor has not yet observed the iRacing process.
    WaitingForProcess,

    /// The process has been observed, so only sim status is checked.
    WaitingForSimulation {
        /// Reusable local sim-status client.
        simulation: Simulation,
    },

    /// Sim status has reported running, so only telemetry connection attempts
    /// are made until one succeeds.
    WaitingForTelemetry,

    /// Telemetry is connected. The monitor owns the connection and checks only
    /// `connection.is_connected()` until it drops.
    Connected {
        /// The single live telemetry connection owned by the backend monitor.
        connection: WindowsConnection,
    },
}

/// Run the Windows lifecycle monitor.
///
/// Unlike a stateless snapshot loop, this function advances through phases and
/// performs only the check relevant to the current phase. It mirrors the example
/// lifecycle while avoiding repeated process checks, repeated sim-status checks,
/// and repeated telemetry connections once the backend has a live connection.
#[cfg(windows)]
fn monitor_windows_connection_state(app: AppHandle, state: Arc<Mutex<IRacingConnectionState>>) {
    let mut phase = MonitorPhase::WaitingForProcess;

    loop {
        phase = match phase {
            MonitorPhase::WaitingForProcess => wait_for_process(&app, &state),
            MonitorPhase::WaitingForSimulation { simulation } => {
                wait_for_simulation(&app, &state, simulation)
            }
            MonitorPhase::WaitingForTelemetry => wait_for_telemetry(&app, &state),
            MonitorPhase::Connected { connection } => {
                monitor_telemetry_connection(&app, &state, connection)
            }
        };

        thread::sleep(POLL_INTERVAL);
    }
}

/// Check only for the iRacing process until it is observed.
#[cfg(windows)]
fn wait_for_process(app: &AppHandle, state: &Arc<Mutex<IRacingConnectionState>>) -> MonitorPhase {
    match is_iracing_process_running() {
        Ok(true) => {
            publish_state(app, state, IRacingConnectionState::waiting_for_sim());
            MonitorPhase::WaitingForSimulation {
                simulation: Simulation::local().with_timeout(SIM_STATUS_TIMEOUT),
            }
        }
        Ok(false) => {
            publish_state(app, state, IRacingConnectionState::disconnected());
            MonitorPhase::WaitingForProcess
        }
        Err(_) => {
            publish_state(app, state, IRacingConnectionState::waiting_for_process());
            MonitorPhase::WaitingForProcess
        }
    }
}

/// Check only the sim-status endpoint while waiting for simulation readiness.
///
/// A successful check advances to telemetry. A failed check steps back to
/// process checking instead of repeatedly probing sim status after the first
/// disconnected result.
#[cfg(windows)]
fn wait_for_simulation(
    app: &AppHandle,
    state: &Arc<Mutex<IRacingConnectionState>>,
    simulation: Simulation,
) -> MonitorPhase {
    if simulation.check_sim_status() {
        publish_state(app, state, IRacingConnectionState::waiting_for_telemetry());
        MonitorPhase::WaitingForTelemetry
    } else {
        publish_state(app, state, IRacingConnectionState::waiting_for_process());
        MonitorPhase::WaitingForProcess
    }
}

/// Attempt to create the single backend-owned telemetry connection.
///
/// A successful connection advances to the connected phase. A failed connection
/// steps back to sim-status checking so the monitor can confirm that the
/// prerequisite is still present before attempting telemetry again.
#[cfg(windows)]
fn wait_for_telemetry(app: &AppHandle, state: &Arc<Mutex<IRacingConnectionState>>) -> MonitorPhase {
    match WindowsConnection::try_connect() {
        Ok(connection) if connection.is_connected() => {
            publish_state(app, state, IRacingConnectionState::connected());
            MonitorPhase::Connected { connection }
        }
        Ok(_) | Err(_) => {
            publish_state(app, state, IRacingConnectionState::waiting_for_sim());
            MonitorPhase::WaitingForSimulation {
                simulation: Simulation::local().with_timeout(SIM_STATUS_TIMEOUT),
            }
        }
    }
}

/// Monitor only the owned telemetry connection while it remains connected.
///
/// When telemetry drops, the monitor walks backward through the minimum checks
/// needed to determine where the lifecycle should resume:
///
/// - If sim status still reports running, keep process and sim connected and
///   return to telemetry connection attempts.
/// - If sim status is unavailable, return to process checking on the next tick.
#[cfg(windows)]
fn monitor_telemetry_connection(
    app: &AppHandle,
    state: &Arc<Mutex<IRacingConnectionState>>,
    connection: WindowsConnection,
) -> MonitorPhase {
    if connection.is_connected() {
        publish_state(app, state, IRacingConnectionState::connected());
        return MonitorPhase::Connected { connection };
    }

    drop(connection);

    let simulation = Simulation::local().with_timeout(SIM_STATUS_TIMEOUT);
    if simulation.check_sim_status() {
        publish_state(app, state, IRacingConnectionState::waiting_for_telemetry());
        return MonitorPhase::WaitingForTelemetry;
    }

    publish_state(app, state, IRacingConnectionState::waiting_for_process());
    MonitorPhase::WaitingForProcess
}
