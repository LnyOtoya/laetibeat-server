use std::sync::{Arc, RwLock};
use std::pin::Pin;
use tokio::sync::{mpsc, broadcast};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use music_backend_engine::Engine;
use music_backend_source::{MusicSource, Song};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Load { song_id: String },
    Play,
    Pause,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybackStatus {
    Idle,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub status: PlaybackStatus,
    pub current: Option<Song>,
    pub position: u64,
    pub duration: u64,
}

#[derive(Debug, Clone)]
pub enum Event {
    StateUpdated(PlayerState),
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Failed to send command: {0}")]
    CommandSendError(#[from] mpsc::error::SendError<Command>),
    #[error("Failed to send event: {0}")]
    EventSendError(#[from] broadcast::error::SendError<Event>),
    #[error("Invalid song ID format")]
    InvalidSongId,
    #[error("Source not found for song ID")]
    SourceNotFound,
    #[error("Error from source: {0}")]
    SourceError(#[from] music_backend_source::SourceError),
}

pub struct Controller {
    command_tx: mpsc::Sender<Command>,
    state: Arc<RwLock<PlayerState>>,
    event_tx: broadcast::Sender<Event>,
    sources: Arc<Vec<Box<dyn MusicSource>>>,
}

impl Controller {
    pub fn new(sources: Vec<Box<dyn MusicSource>>) -> Self {
        let (command_tx, command_rx) = mpsc::channel(10);
        let (event_tx, _) = broadcast::channel(10);
        
        let state = Arc::new(RwLock::new(PlayerState {
            status: PlaybackStatus::Idle,
            current: None,
            position: 0,
            duration: 0,
        }));
        
        let sources = Arc::new(sources);
        
        let controller = Self {
            command_tx,
            state,
            event_tx,
            sources,
        };
        
        controller.spawn_worker(command_rx);
        
        controller
    }
    
    pub async fn send_command(&self, command: Command) -> Result<(), ControllerError> {
        self.command_tx.send(command).await?;
        Ok(())
    }
    
    pub fn get_state(&self) -> PlayerState {
        self.state.read().unwrap().clone()
    }
    
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }
    
    pub fn get_sources(&self) -> &Vec<Box<dyn MusicSource>> {
        &self.sources
    }
    
    fn spawn_worker(&self, mut command_rx: mpsc::Receiver<Command>) {
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let sources = self.sources.clone();
        
        tokio::spawn(async move {
            let mut engine = Engine::new();
            
            while let Some(command) = command_rx.recv().await {
                match command {
                    Command::Load { song_id } => {
                        if let Some((source, _)) = Self::parse_song_id(&song_id) {
                            let source_impl = {
                                sources.iter().find(|s| s.name() == source)
                            };
                            
                            if let Some(source_impl) = source_impl {
                                match Pin::from(source_impl.get_metadata(&song_id)).await {
                                    Ok(song) => {
                                        let mut state_write = state.write().unwrap();
                                        state_write.current = Some(song);
                                        state_write.status = PlaybackStatus::Stopped;
                                        state_write.position = 0;
                                        state_write.duration = 0;
                                        
                                        let new_state = state_write.clone();
                                        let _ = event_tx.send(Event::StateUpdated(new_state));
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to load metadata: {:?}", e);
                                    }
                                }
                            }
                        }
                    },
                    Command::Play => {
                        let song_id = {
                            let state_read = state.read().unwrap();
                            if state_read.current.is_some() && state_read.status != PlaybackStatus::Playing {
                                state_read.current.as_ref().unwrap().id.clone()
                            } else {
                                return;
                            }
                        };
                        
                        if let Some((source, _)) = Self::parse_song_id(&song_id) {
                            if let Some(source_impl) = sources.iter().find(|s| s.name() == source) {
                                match Pin::from(source_impl.get_stream(&song_id)).await {
                                    Ok(stream) => {
                                        engine.play(stream);
                                        
                                        let mut state_write = state.write().unwrap();
                                        state_write.status = PlaybackStatus::Playing;
                                        
                                        let new_state = state_write.clone();
                                        let _ = event_tx.send(Event::StateUpdated(new_state));
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to get stream: {:?}", e);
                                    }
                                }
                            }
                        }
                    },
                    Command::Pause => {
                        engine.pause();
                        
                        let mut state_write = state.write().unwrap();
                        state_write.status = PlaybackStatus::Paused;
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                    },
                    Command::Stop => {
                        engine.stop();
                        
                        let mut state_write = state.write().unwrap();
                        state_write.status = PlaybackStatus::Stopped;
                        state_write.position = 0;
                        
                        let new_state = state_write.clone();
                        let _ = event_tx.send(Event::StateUpdated(new_state));
                    },
                }
            }
        });
    }
    
    fn parse_song_id(song_id: &str) -> Option<(String, String)> {
        if let Some((source, id)) = song_id.split_once(':') {
            Some((source.to_string(), id.to_string()))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
