use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use axum::Router;

use super::settings::{TransportRuntimeStatus, TransportSettings};

pub(super) const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(super) struct ServerHandle {
    label: String,
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
    tracing::debug!(
        transport = label,
        endpoint = %endpoint,
        "transport listener bound"
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let thread_label = label.to_string();
    let join = thread::spawn(move || run(listener, thread_shutdown));
    tracing::debug!(
        transport = %thread_label,
        endpoint = %endpoint,
        "transport thread started"
    );

    Ok(ServerHandle {
        label: label.to_string(),
        endpoint,
        shutdown,
        join: Some(join),
    })
}

pub(super) fn start_axum_transport(
    settings: TransportSettings,
    label: &str,
    scheme: &str,
    build_router: impl FnOnce(Arc<AtomicBool>) -> Router + Send + 'static,
) -> Result<ServerHandle, String> {
    let transport_label = label.to_string();
    start_listener_transport(settings, label, scheme, move |listener, shutdown| {
        run_axum_transport(transport_label, listener, shutdown, build_router);
    })
}

pub(super) fn stop_transport(handle: &mut Option<ServerHandle>) {
    if let Some(mut handle) = handle.take() {
        tracing::debug!(
            transport = %handle.label,
            endpoint = %handle.endpoint,
            "transport shutdown requested"
        );
        handle.shutdown.store(true, Ordering::Release);
        if let Some(join) = handle.join.take() {
            let _ = join.join();
        }
        tracing::debug!(
            transport = %handle.label,
            endpoint = %handle.endpoint,
            "transport stopped"
        );
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
    tracing::debug!(
        transport = label,
        bind_address = %bind_address,
        "binding transport listener"
    );

    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("{label} failed to bind {bind_address}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("{label} failed to enter non-blocking mode: {error}"))?;
    Ok(listener)
}

fn run_axum_transport(
    label: String,
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    build_router: impl FnOnce(Arc<AtomicBool>) -> Router,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        tracing::debug!(
            transport = %label,
            "failed to build transport tokio runtime"
        );
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            tracing::debug!(
                transport = %label,
                "failed to convert transport listener to tokio listener"
            );
            return;
        };

        tracing::debug!(transport = %label, "transport server accepting requests");
        let router = build_router(Arc::clone(&shutdown));
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                while !shutdown.load(Ordering::Acquire) {
                    tokio::time::sleep(ACCEPT_POLL_INTERVAL).await;
                }
            })
            .await;

        match result {
            Ok(()) => tracing::debug!(transport = %label, "transport server exited"),
            Err(error) => tracing::debug!(
                transport = %label,
                error = %error,
                "transport server exited with error"
            ),
        }
    });
}
