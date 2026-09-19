mod db;
mod html;
mod routes;
mod slug;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::db::Database;
use crate::html::HtmlRenderer;
use crate::routes::{create_router, AppState};

#[derive(Parser, Debug)]
#[command(
    name = "btu-server",
    about = "BTU Smart Modulkatalog - Hybrid PWA & SSR Webserver"
)]
struct Args {
    #[arg(short, long, default_value = "8080", env = "PORT")]
    port: u16,

    #[arg(long, default_value = "btu_modules.db", env = "DB_PATH")]
    db: PathBuf,

    #[arg(long, default_value = "frontend/dist", env = "DIST_DIR")]
    dist: PathBuf,

    #[arg(long, default_value = "0.0.0.0", env = "HOST")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,btu_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let db_str = args.db.to_string_lossy();
    tracing::info!("Connecting to SQLite database at: {}", db_str);
    let db = match Database::open(&db_str) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to open SQLite database ({}): {}", db_str, e);
            std::process::exit(1);
        }
    };

    let dist_str = args.dist.to_string_lossy();
    tracing::info!("Initializing HTML renderer from dist: {}", dist_str);
    let renderer = HtmlRenderer::new(&dist_str);

    // Pre-load official study programs
    let programs = Arc::new(db.get_study_programs().unwrap_or_default());
    let stats = db.get_stats().unwrap_or(crate::db::DbStats {
        total_modules: 0,
        total_programs: 0,
    });

    tracing::info!(
        "Loaded database snapshot: {} modules, {} study programs",
        stats.total_modules,
        stats.total_programs
    );

    let state = AppState {
        db,
        renderer,
        dist_dir: args.dist.clone(),
        db_path: args.db.clone(),
        programs,
    };

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    tracing::info!("🚀 BTU Smart Modulkatalog Server listening on http://localhost:{}", args.port);
    tracing::info!("   PWA Shell & No-JS Static SSR active.");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, shutting down server gracefully...");
        },
        _ = terminate => {
            tracing::info!("Received terminate signal, shutting down server gracefully...");
        },
    }
}
