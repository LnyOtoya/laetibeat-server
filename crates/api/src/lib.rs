use axum::{Router, routing::{get, post}, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use music_backend_core::{Controller, Command, PlayerState};
use music_backend_source::Song;

#[derive(Debug, Deserialize)]
pub struct LoadRequest {
    song_id: String,
}

#[derive(Debug, Serialize)]
pub struct LibraryResponse {
    id: String,
    title: String,
    artist: String,
}

pub struct AppState {
    controller: Arc<Controller>,
}

pub fn create_router(controller: Arc<Controller>) -> Router {
    let app_state = AppState {
        controller,
    };
    
    Router::new()
        .route("/api/v1/play", post(play))
        .route("/api/v1/pause", post(pause))
        .route("/api/v1/stop", post(stop))
        .route("/api/v1/load", post(load))
        .route("/api/v1/status", get(status))
        .route("/api/v1/library", get(library))
        .with_state(Arc::new(app_state))
}

async fn play(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Play;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn pause(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Pause;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn stop(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Stop;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn load(State(state): State<Arc<AppState>>, Json(request): Json<LoadRequest>) -> StatusCode {
    let command = Command::Load { song_id: request.song_id };
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn status(State(state): State<Arc<AppState>>) -> Json<PlayerState> {
    let state = state.controller.get_state();
    Json(state)
}

async fn library(State(state): State<Arc<AppState>>) -> Json<Vec<LibraryResponse>> {
    let mut library = Vec::new();
    
    // Get all sources from the controller
    let sources = state.controller.get_sources();
    
    // For each source, get its library
    for source in sources {
        let source_library = std::pin::Pin::from(source.get_library()).await;
        for song in source_library {
            library.push(LibraryResponse {
                id: song.id,
                title: song.title,
                artist: song.artist,
            });
        }
    }
    
    Json(library)
}
