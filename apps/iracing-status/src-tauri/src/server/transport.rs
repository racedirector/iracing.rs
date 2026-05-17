use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::settings::{TransportRuntimeStatus, TransportSettings};

pub(super) const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(super) struct ServerHandle {
    endpoint: String,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

pub(super) fn start_listener_transport(
    settings: TransportSettings,
    label: &str,
    scheme: &str,
    run: impl FnOnce(TcpListener, Arc<AtomicBool>) + Send + 'static,
) -> Result<ServerHandle, String> {
    let listener = bind_listener(&settings, label)?;
    let endpoint = format!("{scheme}://{}:{}", settings.host, settings.port);
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let join = thread::spawn(move || run(listener, thread_shutdown));

    Ok(ServerHandle {
        endpoint,
        shutdown,
        join: Some(join),
    })
}

pub(super) fn stop_transport(handle: &mut Option<ServerHandle>) {
    if let Some(mut handle) = handle.take() {
        handle.shutdown.store(true, Ordering::Release);
        if let Some(join) = handle.join.take() {
            let _ = join.join();
        }
    }
}

pub(super) fn transport_status(handle: &Option<ServerHandle>) -> TransportRuntimeStatus {
    match handle {
        Some(handle) => TransportRuntimeStatus::Running {
            endpoint: handle.endpoint.clone(),
        },
        None => TransportRuntimeStatus::Disabled,
    }
}

fn bind_listener(settings: &TransportSettings, label: &str) -> Result<TcpListener, String> {
    let bind_address = format!("{}:{}", settings.host, settings.port);
    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("{label} failed to bind {bind_address}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("{label} failed to enter non-blocking mode: {error}"))?;
    Ok(listener)
}
