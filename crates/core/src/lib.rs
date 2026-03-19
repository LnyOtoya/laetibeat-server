use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, broadcast, oneshot};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Extension trait for Vec to get last index
pub trait VecExt {
    fn last_index(&self) -> Option<usize>;
}

impl<T> VecExt for Vec<T> {
    fn last_index(&self) -> Option<usize> {
        if self.is_empty() {
            None
        } else {
            Some(self.len() - 1)
        }
    }
}

use music_backend_engine::{Engine, EngineEvent};
use music_backend_source::{Track, SourceManager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub original_order: Vec<Track>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Load { song_id: String },
    Play,
    Pause,
    Stop,
    Next,
    Prev,
    AddToQueue { song_id: String },
    RemoveFromQueue { index: usize },
    ClearQueue,
    SetShuffle { enabled: bool },
    SetRepeat { mode: RepeatMode },
    PlayAtIndex { index: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandResult {
    Ok,
    Error(String),
}

#[derive(Debug)]
pub struct CommandWithResponse {
    pub command: Command,
    pub response_tx: oneshot::Sender<CommandResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybackStatus {
    Idle,
    Playing,
    Paused,
    Stopped,
    Ended,
}

impl std::fmt::Display for PlaybackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackStatus::Idle => write!(f, "Idle"),
            PlaybackStatus::Playing => write!(f, "Playing"),
            PlaybackStatus::Paused => write!(f, "Paused"),
            PlaybackStatus::Stopped => write!(f, "Stopped"),
            PlaybackStatus::Ended => write!(f, "Ended"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub status: PlaybackStatus,
    pub current: Option<Track>,
    pub position: u64,
    pub duration: u64,
    pub queue: Queue,
}

#[derive(Debug, Clone)]
pub enum Event {
    StateUpdated(PlayerState),
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Failed to send command: {0}")]
    CommandSendError(#[from] mpsc::error::SendError<Command>),
    #[error("Failed to send command with response: {0}")]
    CommandWithResponseSendError(#[from] mpsc::error::SendError<CommandWithResponse>),
    #[error("Failed to send event: {0}")]
    EventSendError(#[from] broadcast::error::SendError<Event>),
    #[error("Invalid song ID format")]
    InvalidSongId,
    #[error("Source not found for song ID")]
    SourceNotFound,
    #[error("Queue is empty")]
    QueueEmpty,
    #[error("Index out of bounds")]
    IndexOutOfBounds,
}

pub struct Controller {
    command_tx: mpsc::Sender<CommandWithResponse>,
    state: Arc<RwLock<PlayerState>>,
    event_tx: broadcast::Sender<Event>,
    source_manager: Arc<SourceManager>,
}

impl Controller {
    pub fn new(source_manager: SourceManager) -> Self {
        let (command_tx, command_rx) = mpsc::channel(10);
        let (event_tx, _) = broadcast::channel(10);
        
        let state = Arc::new(RwLock::new(PlayerState {
            status: PlaybackStatus::Idle,
            current: None,
            position: 0,
            duration: 0,
            queue: Queue {
                tracks: Vec::new(),
                current_index: None,
                shuffle: false,
                repeat: RepeatMode::Off,
                original_order: Vec::new(),
            },
        }));
        
        let source_manager = Arc::new(source_manager);
        
        let controller = Self {
            command_tx,
            state,
            event_tx,
            source_manager,
        };
        
        controller.spawn_worker(command_rx);
        
        controller
    }
    
    pub async fn send_command(&self, command: Command) -> Result<oneshot::Receiver<CommandResult>, ControllerError> {
        let (response_tx, response_rx) = oneshot::channel();
        let command_with_response = CommandWithResponse {
            command,
            response_tx,
        };
        self.command_tx.send(command_with_response).await?;
        Ok(response_rx)
    }
    
    pub fn get_state(&self) -> PlayerState {
        self.state.read().unwrap().clone()
    }
    
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }
    
    pub fn get_source_manager(&self) -> &SourceManager {
        &self.source_manager
    }
    
    fn spawn_worker(&self, mut command_rx: mpsc::Receiver<CommandWithResponse>) {
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let source_manager = self.source_manager.clone();
        
        tokio::spawn(async move {
            let _engine = Engine::new();
            
            // 纯后端模式：移除引擎事件处理
            
            while let Some(CommandWithResponse { command, response_tx }) = command_rx.recv().await {
                let current_status = {
                    let state_read = match state.read() {
                        Ok(guard) => guard,
                        Err(e) => {
                            eprintln!("Failed to acquire read lock: {:?}", e);
                            response_tx.send(CommandResult::Error(format!("Failed to acquire read lock: {:?}", e))).ok();
                            continue;
                        }
                    };
                    state_read.status.clone()
                };
                
                let result = match command {
                    Command::Load { song_id } => {
                        // Load is allowed in any state
                        match source_manager.get_track(&song_id).await {
                            Ok(track) => {
                                let mut state_write = match state.write() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        eprintln!("Failed to acquire write lock: {:?}", e);
                                        let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                        response_tx.send(error).ok();
                                        continue;
                                    }
                                };
                                state_write.current = Some(track.clone());
                                state_write.status = PlaybackStatus::Stopped;
                                state_write.position = 0;
                                state_write.duration = 0;
                                
                                // Add to queue if not already present
                                if !state_write.queue.tracks.iter().any(|t| t.id == song_id) {
                                    state_write.queue.tracks.push(track.clone());
                                    state_write.queue.original_order.push(track);
                                    state_write.queue.current_index = Some(state_write.queue.tracks.len() - 1);
                                }
                                
                                let new_state = state_write.clone();
                                let _ = event_tx.send(Event::StateUpdated(new_state));
                                CommandResult::Ok
                            }
                            Err(e) => {
                                eprintln!("Failed to load track: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to load track: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        }
                    },
                    Command::Play => {
                        match current_status {
                            PlaybackStatus::Stopped | PlaybackStatus::Paused | PlaybackStatus::Ended => {
                                let song_id = {
                                    let state_read = match state.read() {
                                        Ok(guard) => guard,
                                        Err(e) => {
                                            eprintln!("Failed to acquire read lock: {:?}", e);
                                            response_tx.send(CommandResult::Error(format!("Failed to acquire read lock: {:?}", e))).ok();
                                            continue;
                                        }
                                    };
                                    if state_read.current.is_some() {
                                        state_read.current.as_ref().unwrap().id.clone()
                                    } else {
                                        response_tx.send(CommandResult::Error("No song loaded".to_string())).ok();
                                        continue;
                                    }
                                };
                                
                                // 纯后端模式：不执行实际播放，只更新状态
                                let mut state_write = match state.write() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        eprintln!("Failed to acquire write lock: {:?}", e);
                                        let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                        response_tx.send(error).ok();
                                        continue;
                                    }
                                };
                                state_write.status = PlaybackStatus::Playing;
                                
                                let new_state = state_write.clone();
                                let _ = event_tx.send(Event::StateUpdated(new_state));
                                CommandResult::Ok
                            }
                            PlaybackStatus::Playing => {
                                let error = CommandResult::Error("Already playing".to_string());
                                response_tx.send(error).ok();
                                continue;
                            }
                            PlaybackStatus::Idle => {
                                let error = CommandResult::Error("No song loaded".to_string());
                                response_tx.send(error).ok();
                                continue;
                            }
                        }
                    },
                    Command::Pause => {
                        match current_status {
                            PlaybackStatus::Playing => {
                                // 纯后端模式：不执行实际暂停，只更新状态
                                let mut state_write = match state.write() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        eprintln!("Failed to acquire write lock: {:?}", e);
                                        let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                        response_tx.send(error).ok();
                                        continue;
                                    }
                                };
                                state_write.status = PlaybackStatus::Paused;
                                
                                let new_state = state_write.clone();
                                let _ = event_tx.send(Event::StateUpdated(new_state));
                                CommandResult::Ok
                            }
                            _ => {
                                let error = CommandResult::Error(format!("Cannot pause in {} state", current_status));
                                response_tx.send(error).ok();
                                continue;
                            }
                        }
                    },
                    Command::Stop => {
                        match current_status {
                            PlaybackStatus::Playing | PlaybackStatus::Paused | PlaybackStatus::Ended => {
                                // 纯后端模式：不执行实际停止，只更新状态
                                let mut state_write = match state.write() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        eprintln!("Failed to acquire write lock: {:?}", e);
                                        let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                        response_tx.send(error).ok();
                                        continue;
                                    }
                                };
                                state_write.status = PlaybackStatus::Stopped;
                                state_write.position = 0;
                                
                                let new_state = state_write.clone();
                                let _ = event_tx.send(Event::StateUpdated(new_state));
                                CommandResult::Ok
                            }
                            _ => {
                                let error = CommandResult::Error(format!("Cannot stop in {} state", current_status));
                                response_tx.send(error).ok();
                                continue;
                            }
                        }
                    },
                    Command::Next => {
                        // Next is allowed in any state with a queue
                        let result = Self::handle_next(&state, &event_tx, &source_manager).await;
                        if let CommandResult::Error(_) = result {
                            response_tx.send(result).ok();
                            continue;
                        }
                        result
                    },
                    Command::Prev => {
                        // Prev is allowed in any state with a queue
                        let result = Self::handle_prev(&state, &event_tx, &source_manager).await;
                        if let CommandResult::Error(_) = result {
                            response_tx.send(result).ok();
                            continue;
                        }
                        result
                    },
                    Command::AddToQueue { song_id } => {
                        // AddToQueue is allowed in any state
                        match source_manager.get_track(&song_id).await {
                            Ok(track) => {
                                let mut state_write = match state.write() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        eprintln!("Failed to acquire write lock: {:?}", e);
                                        let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                        response_tx.send(error).ok();
                                        continue;
                                    }
                                };
                                if !state_write.queue.tracks.iter().any(|t| t.id == song_id) {
                                    state_write.queue.tracks.push(track.clone());
                                    state_write.queue.original_order.push(track);
                                }
                                
                                let new_state = state_write.clone();
                                let _ = event_tx.send(Event::StateUpdated(new_state));
                                CommandResult::Ok
                            }
                            Err(e) => {
                                eprintln!("Failed to load track: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to load track: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        }
                    },
                    Command::RemoveFromQueue { index } => {
                        // RemoveFromQueue is allowed in any state
                        let mut state_write = match state.write() {
                            Ok(guard) => guard,
                            Err(e) => {
                                eprintln!("Failed to acquire write lock: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        if index < state_write.queue.tracks.len() {
                            state_write.queue.tracks.remove(index);
                            // Create a set of remaining track IDs for efficient lookup
                            let track_ids: std::collections::HashSet<_> = state_write.queue.tracks.iter().map(|t| t.id.clone()).collect();
                            state_write.queue.original_order.retain(|t| track_ids.contains(&t.id));
                            if let Some(current_index) = state_write.queue.current_index {
                                if current_index >= state_write.queue.tracks.len() {
                                    state_write.queue.current_index = state_write.queue.tracks.last_index();
                                } else if current_index > index {
                                    state_write.queue.current_index = Some(current_index - 1);
                                }
                            }
                            
                            let new_state = state_write.clone();
                            let _ = event_tx.send(Event::StateUpdated(new_state));
                            CommandResult::Ok
                        } else {
                            let error = CommandResult::Error("Index out of bounds".to_string());
                            response_tx.send(error).ok();
                            continue;
                        }
                    },
                    Command::ClearQueue => {
                        // ClearQueue is allowed in any state
                        let mut state_write = match state.write() {
                            Ok(guard) => guard,
                            Err(e) => {
                                eprintln!("Failed to acquire write lock: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        state_write.queue.tracks.clear();
                        state_write.queue.original_order.clear();
                        state_write.queue.current_index = None;
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                        CommandResult::Ok
                    },
                    Command::SetShuffle { enabled } => {
                        // SetShuffle is allowed in any state
                        let mut state_write = match state.write() {
                            Ok(guard) => guard,
                            Err(e) => {
                                eprintln!("Failed to acquire write lock: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        state_write.queue.shuffle = enabled;
                        if enabled {
                            // Shuffle tracks while keeping current song at current index
                            if let Some(current_index) = state_write.queue.current_index {
                                if current_index < state_write.queue.tracks.len() {
                                    let current_track = state_write.queue.tracks.remove(current_index);
                                    use rand::seq::SliceRandom;
                                    let mut rng = rand::thread_rng();
                                    state_write.queue.tracks.shuffle(&mut rng);
                                    state_write.queue.tracks.insert(current_index, current_track);
                                }
                            } else if !state_write.queue.tracks.is_empty() {
                                use rand::seq::SliceRandom;
                                let mut rng = rand::thread_rng();
                                state_write.queue.tracks.shuffle(&mut rng);
                            }
                        } else {
                            // Restore original order
                            state_write.queue.tracks = state_write.queue.original_order.clone();
                        }
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                        CommandResult::Ok
                    },
                    Command::SetRepeat { mode } => {
                        // SetRepeat is allowed in any state
                        let mut state_write = match state.write() {
                            Ok(guard) => guard,
                            Err(e) => {
                                eprintln!("Failed to acquire write lock: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        state_write.queue.repeat = mode;
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                        CommandResult::Ok
                    },
                    Command::PlayAtIndex { index } => {
                        // PlayAtIndex is allowed in any state with a queue
                        let (track, _track_id) = {
                            let state_read = match state.read() {
                                Ok(guard) => guard,
                                Err(e) => {
                                    eprintln!("Failed to acquire read lock: {:?}", e);
                                    let error = CommandResult::Error(format!("Failed to acquire read lock: {:?}", e));
                                    response_tx.send(error).ok();
                                    continue;
                                }
                            };
                            if index < state_read.queue.tracks.len() {
                                let track = state_read.queue.tracks[index].clone();
                                let track_id = track.id.clone();
                                (track, track_id)
                            } else {
                                drop(state_read);
                                let error = CommandResult::Error("Index out of bounds".to_string());
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        
                        // 纯后端模式：不执行实际播放，只更新状态
                        let mut state_write = match state.write() {
                            Ok(guard) => guard,
                            Err(e) => {
                                eprintln!("Failed to acquire write lock: {:?}", e);
                                let error = CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                                response_tx.send(error).ok();
                                continue;
                            }
                        };
                        state_write.current = Some(track);
                        state_write.queue.current_index = Some(index);
                        state_write.status = PlaybackStatus::Playing;
                        state_write.position = 0;
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                        CommandResult::Ok
                    },
                };
                
                // Send response
                if response_tx.send(result).is_err() {
                    eprintln!("Failed to send command response: receiver dropped");
                }
            }
        });
    }
    
    async fn handle_next(
        state: &Arc<RwLock<PlayerState>>,
        event_tx: &broadcast::Sender<Event>,
        _source_manager: &Arc<SourceManager>,
    ) -> CommandResult {
        let (next_index, track) = {
            let state_read = match state.read() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to acquire read lock: {:?}", e);
                    return CommandResult::Error(format!("Failed to acquire read lock: {:?}", e));
                }
            };
            let queue = &state_read.queue;
            
            let next_index = match queue.current_index {
                Some(current) => {
                    if current + 1 < queue.tracks.len() {
                        Some(current + 1)
                    } else if queue.repeat == RepeatMode::All && !queue.tracks.is_empty() {
                        Some(0)
                    } else {
                        None
                    }
                }
                None if !queue.tracks.is_empty() => Some(0),
                _ => None,
            };
            
            if let Some(index) = next_index {
                let track = queue.tracks[index].clone();
                (next_index, Some(track))
            } else {
                (None, None)
            }
        };
        
        if let (Some(index), Some(track)) = (next_index, track) {
            // 纯后端模式：不执行实际播放，只更新状态
            let mut state_write = match state.write() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to acquire write lock: {:?}", e);
                    return CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                }
            };
            state_write.current = Some(track);
            state_write.queue.current_index = Some(index);
            state_write.status = PlaybackStatus::Playing;
            state_write.position = 0;
            
            let new_state = state_write.clone();
            let _ = event_tx.send(Event::StateUpdated(new_state));
            CommandResult::Ok
        } else {
            CommandResult::Error("No next song available".to_string())
        }
    }
    
    async fn handle_prev(
        state: &Arc<RwLock<PlayerState>>,
        event_tx: &broadcast::Sender<Event>,
        _source_manager: &Arc<SourceManager>,
    ) -> CommandResult {
        let (prev_index, track) = {
            let state_read = match state.read() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to acquire read lock: {:?}", e);
                    return CommandResult::Error(format!("Failed to acquire read lock: {:?}", e));
                }
            };
            let queue = &state_read.queue;
            
            let prev_index = match queue.current_index {
                Some(current) => {
                    if current > 0 {
                        Some(current - 1)
                    } else if queue.repeat == RepeatMode::All && !queue.tracks.is_empty() {
                        queue.tracks.last_index()
                    } else {
                        None
                    }
                }
                None if !queue.tracks.is_empty() => queue.tracks.last_index(),
                _ => None,
            };
            
            if let Some(index) = prev_index {
                let track = queue.tracks[index].clone();
                (prev_index, Some(track))
            } else {
                (None, None)
            }
        };
        
        if let (Some(index), Some(track)) = (prev_index, track) {
            // 纯后端模式：不执行实际播放，只更新状态
            let mut state_write = match state.write() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to acquire write lock: {:?}", e);
                    return CommandResult::Error(format!("Failed to acquire write lock: {:?}", e));
                }
            };
            state_write.current = Some(track);
            state_write.queue.current_index = Some(index);
            state_write.status = PlaybackStatus::Playing;
            state_write.position = 0;
            
            let new_state = state_write.clone();
            let _ = event_tx.send(Event::StateUpdated(new_state));
            CommandResult::Ok
        } else {
            CommandResult::Error("No previous song available".to_string())
        }
    }
}

#[cfg(test)]
mod tests;