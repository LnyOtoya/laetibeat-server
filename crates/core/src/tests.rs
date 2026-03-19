use super::*;
use music_backend_source::{MusicSource, Track, AudioStream, SourceError, SourceManager};
use std::sync::Arc;
use async_trait::async_trait;

// Mock MusicSource for testing
#[derive(Clone)]
struct MockSource {
    tracks: Vec<Track>,
}

impl MockSource {
    fn new() -> Self {
        Self {
            tracks: vec![
                Track {
                    id: "local:test1".to_string(),
                    title: "Test Song 1".to_string(),
                    artist: "Test Artist 1".to_string(),
                    album: "Test Album 1".to_string(),
                    duration: 180000, // 3 minutes
                    source: "local".to_string(),
                },
            ],
        }
    }
}

#[async_trait]
impl MusicSource for MockSource {
    fn name(&self) -> &str {
        "local"
    }
    
    async fn get_track(&self, id: &str) -> Result<Track, SourceError> {
        self.tracks.iter()
            .find(|track| track.id == id)
            .cloned()
            .ok_or(SourceError::TrackNotFound)
    }
    
    async fn get_stream(&self, _id: &str) -> Result<AudioStream, SourceError> {
        // For testing, we'll return an error since we're not testing actual playback
        Err(SourceError::TrackNotFound)
    }
    
    async fn list(&self) -> Result<Vec<Track>, SourceError> {
        Ok(self.tracks.clone())
    }
}

#[tokio::test]
async fn test_controller_load_play_pause_stop() {
    // Create mock source
    let mock_source = MockSource::new();
    let sources = vec![Arc::new(mock_source) as Arc<_>];
    let source_manager = SourceManager::new(sources);
    
    // Create controller
    let controller = Controller::new(source_manager);
    
    // Test Load
    let load_command = Command::Load { song_id: "local:test1".to_string() };
    controller.send_command(load_command).await.unwrap();
    
    // Wait a bit for the command to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let state = controller.get_state();
    assert_eq!(state.status, PlaybackStatus::Stopped);
    assert!(state.current.is_some());
    assert_eq!(state.current.as_ref().unwrap().id, "local:test1");
    
    // Test Stop
    let stop_command = Command::Stop;
    controller.send_command(stop_command).await.unwrap();
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let state = controller.get_state();
    assert_eq!(state.status, PlaybackStatus::Stopped);
    assert_eq!(state.position, 0);
}