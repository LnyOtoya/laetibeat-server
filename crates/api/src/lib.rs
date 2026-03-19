use axum::{Router, routing::{get, post}, Json, extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}, Path}};
use axum::http::{StatusCode, Response, header::HeaderMap};
use axum::body::{Body};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::pin::Pin;
use tokio::sync::{broadcast, Mutex};
use tokio::io::{AsyncRead, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use music_backend_source::AudioStream;

use music_backend_core::{Controller, Command, PlayerState, CommandResult, RepeatMode, Event};

// 统一响应结构
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub state: PlayerState,
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

// 请求结构
#[derive(Debug, Deserialize)]
pub struct LoadRequest {
    song_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AddToQueueRequest {
    song_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveFromQueueRequest {
    index: usize,
}

#[derive(Debug, Deserialize)]
pub struct PlayAtIndexRequest {
    index: usize,
}

#[derive(Debug, Deserialize)]
pub struct SetShuffleRequest {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetRepeatRequest {
    mode: RepeatMode,
}

// 响应结构
#[derive(Debug, Serialize)]
pub struct LibraryResponse {
    id: String,
    title: String,
    artist: String,
}

pub struct AppState {
    controller: Arc<Controller>,
    ws_clients: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<Message>>>>,
    state_broadcast: broadcast::Sender<PlayerState>,
}

pub fn create_router(controller: Arc<Controller>) -> Router {
    // Create broadcast channel for state updates
    let (state_broadcast, _) = broadcast::channel(10);
    
    let app_state = AppState {
        controller: controller.clone(),
        ws_clients: Arc::new(Mutex::new(Vec::new())),
        state_broadcast: state_broadcast.clone(),
    };
    
    // Spawn task to listen for state updates and broadcast to WebSocket clients
    let controller_clone = controller.clone();
    let state_broadcast_clone = state_broadcast;
    tokio::spawn(async move {
        let mut event_rx = controller_clone.subscribe_events();
        while let Ok(event) = event_rx.recv().await {
            match event {
                Event::StateUpdated(state) => {
                    let _ = state_broadcast_clone.send(state);
                }
            }
        }
    });
    
    let app_state_arc = Arc::new(app_state);
    
    Router::new()
        // V1 接口（保持兼容）
        .route("/api/v1/play", post(play_v1))
        .route("/api/v1/pause", post(pause_v1))
        .route("/api/v1/stop", post(stop_v1))
        .route("/api/v1/load", post(load_v1))
        .route("/api/v1/status", get(status_v1))
        .route("/api/v1/library", get(library_v1))
        // V2 接口
        .route("/api/v2/play", post(play_v2))
        .route("/api/v2/pause", post(pause_v2))
        .route("/api/v2/stop", post(stop_v2))
        .route("/api/v2/load", post(load_v2))
        .route("/api/v2/next", post(next_v2))
        .route("/api/v2/prev", post(prev_v2))
        .route("/api/v2/queue/add", post(add_to_queue_v2))
        .route("/api/v2/queue/remove", post(remove_from_queue_v2))
        .route("/api/v2/queue/clear", post(clear_queue_v2))
        .route("/api/v2/queue/shuffle", post(set_shuffle_v2))
        .route("/api/v2/queue/repeat", post(set_repeat_v2))
        .route("/api/v2/queue/play", post(play_at_index_v2))
        .route("/api/v2/status", get(status_v2))
        .route("/api/v2/library", get(library_v2))
        // 音频流接口
        .route("/api/v2/stream/:id", get(stream_handler))
        // WebSocket route
        .route("/ws/status", get(ws_status))
        .with_state(app_state_arc)
}

// V1 接口实现（保持兼容）
async fn play_v1(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Play;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn pause_v1(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Pause;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn stop_v1(State(state): State<Arc<AppState>>) -> StatusCode {
    let command = Command::Stop;
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn load_v1(State(state): State<Arc<AppState>>, Json(request): Json<LoadRequest>) -> StatusCode {
    let command = Command::Load { song_id: request.song_id };
    if state.controller.send_command(command).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn status_v1(State(state): State<Arc<AppState>>) -> Json<PlayerState> {
    let state = state.controller.get_state();
    Json(state)
}

async fn library_v1(State(state): State<Arc<AppState>>) -> Json<Vec<LibraryResponse>> {
    let mut library = Vec::new();
    
    // Get library from source manager
    if let Ok(tracks) = state.controller.get_source_manager().list().await {
        for track in tracks {
            library.push(LibraryResponse {
                id: track.id,
                title: track.title,
                artist: track.artist,
            });
        }
    }
    
    Json(library)
}

// V2 接口实现
async fn play_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::Play;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "PLAY_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn pause_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::Pause;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "PAUSE_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn stop_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::Stop;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "STOP_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn load_v2(State(state): State<Arc<AppState>>, Json(request): Json<LoadRequest>) -> Json<ApiResponse<()>> {
    let command = Command::Load { song_id: request.song_id };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "LOAD_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn next_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::Next;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "NEXT_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn prev_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::Prev;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "PREV_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn add_to_queue_v2(State(state): State<Arc<AppState>>, Json(request): Json<AddToQueueRequest>) -> Json<ApiResponse<()>> {
    let command = Command::AddToQueue { song_id: request.song_id };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "ADD_TO_QUEUE_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn remove_from_queue_v2(State(state): State<Arc<AppState>>, Json(request): Json<RemoveFromQueueRequest>) -> Json<ApiResponse<()>> {
    let command = Command::RemoveFromQueue { index: request.index };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "REMOVE_FROM_QUEUE_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn clear_queue_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let command = Command::ClearQueue;
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "CLEAR_QUEUE_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn set_shuffle_v2(State(state): State<Arc<AppState>>, Json(request): Json<SetShuffleRequest>) -> Json<ApiResponse<()>> {
    let command = Command::SetShuffle { enabled: request.enabled };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "SET_SHUFFLE_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn set_repeat_v2(State(state): State<Arc<AppState>>, Json(request): Json<SetRepeatRequest>) -> Json<ApiResponse<()>> {
    let command = Command::SetRepeat { mode: request.mode };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "SET_REPEAT_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn play_at_index_v2(State(state): State<Arc<AppState>>, Json(request): Json<PlayAtIndexRequest>) -> Json<ApiResponse<()>> {
    let command = Command::PlayAtIndex { index: request.index };
    match state.controller.send_command(command).await {
        Ok(mut response_rx) => {
            match response_rx.await {
                Ok(CommandResult::Ok) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: true,
                        data: None,
                        state,
                        error: None,
                    })
                }
                Ok(CommandResult::Error(msg)) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "PLAY_AT_INDEX_ERROR".to_string(),
                            message: msg,
                        }),
                    })
                }
                Err(e) => {
                    let state = state.controller.get_state();
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        state,
                        error: Some(ApiError {
                            code: "INTERNAL_ERROR".to_string(),
                            message: format!("Failed to receive response: {:?}", e),
                        }),
                    })
                }
            }
        }
        Err(e) => {
            let state = state.controller.get_state();
            Json(ApiResponse {
                success: false,
                data: None,
                state,
                error: Some(ApiError {
                    code: "INTERNAL_ERROR".to_string(),
                    message: format!("Failed to send command: {:?}", e),
                }),
            })
        }
    }
}

async fn status_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let state = state.controller.get_state();
    Json(ApiResponse {
        success: true,
        data: None,
        state,
        error: None,
    })
}

async fn library_v2(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<LibraryResponse>>> {
    let mut library = Vec::new();
    
    // Get library from source manager
    if let Ok(tracks) = state.controller.get_source_manager().list().await {
        for track in tracks {
            library.push(LibraryResponse {
                id: track.id,
                title: track.title,
                artist: track.artist,
            });
        }
    }
    
    let state = state.controller.get_state();
    Json(ApiResponse {
        success: true,
        data: Some(library),
        state,
        error: None,
    })
}

// WebSocket handler for status updates
async fn ws_status(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    // Wrap socket in Arc<Mutex> to share between tasks
    let socket = Arc::new(tokio::sync::Mutex::new(socket));
    
    // Create a channel for this client
    let (tx, _rx) = tokio::sync::mpsc::channel(10);
    
    // Add client to the list
    {
        let mut clients = state.ws_clients.lock().await;
        clients.push(tx);
    }
    
    // Subscribe to state updates
    let mut state_rx = state.state_broadcast.subscribe();
    
    // Send initial state
    let initial_state = state.controller.get_state();
    if let Ok(json) = serde_json::to_string(&initial_state) {
        if socket.lock().await.send(Message::Text(json)).await.is_err() {
            // Client disconnected
            let mut clients = state.ws_clients.lock().await;
            clients.retain(|c| !c.is_closed());
            return;
        }
    }
    
    // Spawn task to receive state updates and send to client
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        while let Ok(new_state) = state_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&new_state) {
                if socket_clone.lock().await.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Handle incoming messages from client (if any)
    while let Some(msg) = socket.lock().await.recv().await {
        match msg {
            Ok(Message::Ping(ping)) => {
                if socket.lock().await.send(Message::Pong(ping)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Ignore pongs
            }
            Ok(Message::Text(_)) => {
                // Handle text messages if needed
            }
            Ok(Message::Binary(_)) => {
                // Handle binary messages if needed
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(_) => {
                break;
            }
        }
    }
    
    // Remove client from the list
    let mut clients = state.ws_clients.lock().await;
    clients.retain(|c| !c.is_closed());
}

/// 解析 Range 头部
fn parse_range(range_header: &str, file_size: u64) -> Result<(u64, u64), StatusCode> {
    // 检查 Range 头部格式
    if !range_header.starts_with("bytes=") {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let range_str = &range_header[6..];
    let parts: Vec<&str> = range_str.split('-').collect();
    
    if parts.len() != 2 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let start = match parts[0].parse::<u64>() {
        Ok(s) => s,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };
    
    let end = match parts[1] {
        "" => file_size - 1,
        s => match s.parse::<u64>() {
            Ok(e) => e,
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        },
    };
    
    // 验证 Range 是否有效
    if start > end || end >= file_size {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    
    Ok((start, end))
}

/// 处理音频流请求
async fn stream_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    // 获取音频流
    let stream_result = state.controller.get_source_manager().get_stream(&id).await;
    let stream = match stream_result {
        Ok(stream) => stream,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };
    
    // 尝试获取支持 seek 的流
    if let Some(mut seekable_stream) = stream.into_async_seek() {
        // 获取文件大小
        let file_size = match seekable_stream.seek(std::io::SeekFrom::End(0)).await {
            Ok(size) => size,
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        };
        
        // 重置到文件开头
        if seekable_stream.seek(std::io::SeekFrom::Start(0)).await.is_err() {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        
        // 检查是否有 Range 头部
        if let Some(range_header) = headers.get("range") {
            if let Ok(range_str) = range_header.to_str() {
                match parse_range(range_str, file_size) {
                    Ok((start, end)) => {
                        // 定位到指定位置
                        if seekable_stream.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        
                        // 计算响应长度
                        let length = end - start + 1;
                        
                        // 创建有限长度的流
                        let limited_reader = LimitedReader::new(seekable_stream, length);
                        let stream = ReaderStream::new(limited_reader);
                        
                        // 创建响应
                        let response = Response::builder()
                            .status(StatusCode::PARTIAL_CONTENT)
                            .header("Accept-Ranges", "bytes")
                            .header("Content-Length", length.to_string())
                            .header("Content-Type", "audio/mpeg") // 简化处理，实际应该根据文件类型设置
                            .header("Content-Range", format!("bytes {}-{}/{}", start, end, file_size))
                            .body(Body::wrap_stream(stream))
                            .unwrap();
                        
                        return Ok(response);
                    }
                    Err(status) => return Err(status),
                }
            }
        }
        
        // 没有 Range 头部，返回完整文件
        let stream = ReaderStream::new(seekable_stream);
        
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Accept-Ranges", "bytes")
            .header("Content-Length", file_size.to_string())
            .header("Content-Type", "audio/mpeg") // 简化处理，实际应该根据文件类型设置
            .body(Body::wrap_stream(stream))
            .unwrap();
        
        Ok(response)
    } else {
        // 重新获取流，因为之前的流已经被移动
        let stream = match state.controller.get_source_manager().get_stream(&id).await {
            Ok(stream) => stream,
            Err(_) => return Err(StatusCode::NOT_FOUND),
        };
        
        // 不支持 seek 的流，直接返回完整流
        let async_read = stream.into_async_read();
        let stream = ReaderStream::new(async_read);
        
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "audio/mpeg") // 简化处理，实际应该根据文件类型设置
            .body(Body::wrap_stream(stream))
            .unwrap();
        
        Ok(response)
    }
}

/// 有限长度的读取器
struct LimitedReader<R: AsyncRead + Unpin> {
    reader: R,
    remaining: u64,
}

impl<R: AsyncRead + Unpin> LimitedReader<R> {
    fn new(reader: R, limit: u64) -> Self {
        Self {
            reader,
            remaining: limit,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for LimitedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.remaining == 0 {
            return std::task::Poll::Ready(Ok(()));
        }
        
        let max_capacity = std::cmp::min(buf.remaining() as u64, self.remaining) as usize;
        
        // 创建一个临时缓冲区
        let mut temp_buf = vec![0; max_capacity];
        let mut temp_read_buf = tokio::io::ReadBuf::new(&mut temp_buf);
        
        match Pin::new(&mut self.reader).poll_read(cx, &mut temp_read_buf) {
            std::task::Poll::Ready(Ok(())) => {
                let bytes_read = temp_read_buf.filled().len();
                self.remaining -= bytes_read as u64;
                
                // 将读取的数据复制到原始 buf
                buf.put_slice(temp_read_buf.filled());
                
                std::task::Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

/// 将 AudioStream 转换为 axum HTTP Body 的高性能方案
pub fn into_body(stream: AudioStream) -> Body {
    // 将 AudioStream 转换为 AsyncRead
    let async_read = stream.into_async_read();
    
    // 使用 ReaderStream 将 AsyncRead 转换为 Stream<Item = Result<Bytes, Error>>
    let reader_stream = ReaderStream::new(async_read);
    
    // 使用 Body::wrap_stream 将 Stream 转换为 Body
    Body::wrap_stream(reader_stream)
}

/*
说明：

1. ReaderStream 工作原理：
   - ReaderStream 是 tokio-util 提供的工具，将 AsyncRead 转换为 Stream
   - 它内部维护一个缓冲区，当缓冲区有数据时，会产生一个包含数据的 Stream 项
   - 当读取到 EOF 时，Stream 会结束
   - 当遇到错误时，Stream 会产生一个包含错误的 Stream 项

2. 为什么适合大文件：
   - 零拷贝优先：ReaderStream 使用缓冲区直接读取数据，避免了不必要的数据拷贝
   - 分块传输：数据会被分成多个块进行传输，而不是一次性加载整个文件到内存
   - backpressure 支持：当下游处理速度跟不上时，ReaderStream 会暂停读取，避免内存堆积
   - 异步处理：使用异步 IO，不会阻塞服务器线程

3. 与直接 read 的区别：
   - 直接 read 会一次性将数据读入内存，对于大文件会占用大量内存
   - ReaderStream 采用流式处理，内存占用恒定，与文件大小无关
   - 直接 read 是阻塞操作，会阻塞服务器线程
   - ReaderStream 是异步操作，不会阻塞服务器线程
   - 直接 read 无法支持分块传输和 backpressure
   - ReaderStream 天然支持分块传输和 backpressure
*/
