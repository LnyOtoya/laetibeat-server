use axum::{Router, routing::{get, post}, Json, extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use music_backend_core::{Controller, Command, PlayerState, CommandResult, RepeatMode, Event};
use music_backend_source::Song;

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
