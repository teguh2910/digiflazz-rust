//! Service entrypoint.

use std::net::SocketAddr;
use std::process::ExitCode;

use digiflazz_service::{routes, AppState, Config};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let production = config.production;
    let port = config.port;

    let state = match AppState::new(config) {
        Ok(state) => state,
        Err(error) => {
            tracing::error!("failed to build Digiflazz client: {error}");
            return ExitCode::FAILURE;
        }
    };

    let app = routes::router(state);

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("failed to bind {address}: {error}");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(
        %address,
        environment = if production { "production" } else { "development" },
        "digiflazz-service listening"
    );

    if production {
        tracing::warn!("running against production credentials; transactions spend real balance");
    }

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("server error: {error}");
        return ExitCode::FAILURE;
    }

    tracing::info!("shutdown complete");
    ExitCode::SUCCESS
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,digiflazz_service=debug"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
