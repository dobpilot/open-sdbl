use std::sync::Arc;

use open_sdbl_trino::cache::MetadataCache;
use open_sdbl_trino::config::ServiceConfig;
use open_sdbl_trino::postgres::create_pool;
use open_sdbl_trino::server::{AppState, router};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("OPEN_SDBL_LOG")
                .unwrap_or_else(|_| EnvFilter::new("open_sdbl_trino=info")),
        )
        .init();
    if let Err(error) = run().await {
        error!(error = %error, "open-sdbl-trino terminated");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServiceConfig::from_env()?;
    let pool = create_pool(&config)?;
    let cache = MetadataCache::new(
        pool.clone(),
        config.metadata_cache_ttl,
        config.config_decode_batch_size,
    );
    // The first load is eager so readiness and bad database configuration are
    // visible before Kubernetes sends connector traffic.
    Arc::clone(&cache).get().await?;
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    info!(listen = %config.listen, "open-sdbl-trino listening");
    axum::serve(listener, router(AppState::new(cache, pool, config)))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let control_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            error!(error = %error, "cannot install Ctrl-C handler");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => error!(error = %error, "cannot install SIGTERM handler"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = control_c => {},
        () = terminate => {},
    }
}
