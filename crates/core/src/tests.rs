use super::*;
use music_backend_source::{MusicSource, Song, SourceError};
use music_backend_engine::AudioInput;
use std::future::Future;

// Mock MusicSource for testing
#[derive(Clone)]
struct MockSource {
    songs: Vec<Song>,
}

impl MockSource {
    fn new() -> Self {
        Self {
            songs: vec![
                Song {
                    id: "local:test1".to_string(),
                    title: "Test Song 1".to_string(),
                    artist: "Test Artist 1".to_string(),
                    album: Some("Test Album 1".to_string()),
                    duration: Some(180000), // 3 minutes
                    source: "local".to_string(),
                },
            ],
        }
    }
}

impl MusicSource for MockSource {
    fn name(&self) -> &str {
        "local"
    }
    
    fn get_stream(&self, _song_id: &str) -> Box<dyn Future<Output = Result<AudioInput, SourceError>> + Send> {
        // For testing, we'll return an error since we're not testing actual playback
        Box::new(async move {
            Err(SourceError::SongNotFound)
        })
    }
    
    fn get_metadata(&self, song_id: &str) -> Box<dyn Future<Output = Result<Song, SourceError>> + Send> {
        let self_clone = self.clone();
        let song_id = song_id.to_string();
        
        Box::new(async move {
            self_clone.songs.iter()
                .find(|song| song.id == song_id)
                .cloned()
                .ok_or(SourceError::SongNotFound)
        })
    }
    
    fn get_library(&self) -> Box<dyn Future<Output = Vec<Song>> + Send> {
        let self_clone = self.clone();
        
        Box::new(async move {
            self_clone.songs.clone()
        })
    }
}

#[tokio::test]
async fn test_controller_load_play_pause_stop() {
    // Create mock source
    let mock_source = MockSource::new();
    let sources = vec![Box::new(mock_source) as Box<_>];
    
    // Create controller
    let controller = Controller::new(sources);
    
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
