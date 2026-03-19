use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use axum::Server;
use music_backend_api::create_router;
use music_backend_core::Controller;
use music_backend_source::{LocalSource, SourceManager};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    
    info!("Starting music backend server...");
    
    // Create LocalSource
    let local_source = match LocalSource::new().await {
        Ok(source) => {
            info!("Local source initialized successfully");
            source
        }
        Err(e) => {
            warn!("Failed to initialize local source: {:?}", e);
            return;
        }
    };
    
    // Create SourceManager and add LocalSource
    let sources = vec![Arc::new(local_source) as Arc<_>];
    let source_manager = SourceManager::new(sources);
    
    // Create Controller with SourceManager
    let controller = Arc::new(Controller::new(source_manager));
    
    // Create router
    let router = create_router(controller);
    
    // Start server
    let addr = "127.0.0.1:3000";
    
    info!("Server listening on {}", addr);
    info!("API endpoints:");
    info!("GET  /api/v1/status - Get player status");
    info!("GET  /api/v1/library - Get music library");
    info!("POST /api/v1/load - Load a song");
    info!("POST /api/v1/play - Play music");
    info!("POST /api/v1/pause - Pause music");
    info!("POST /api/v1/stop - Stop music");
    
    axum::Server::bind(&addr.parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}