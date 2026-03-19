use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::future::Future;
use std::marker::Unpin;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use lofty::{AudioFile, TaggedFile, TaggedFileExt, LoftyError, Accessor, ParseOptions};
use sha1::{Sha1, Digest};

use music_backend_engine::AudioInput;

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Metadata error: {0}")]
    MetadataError(#[from] LoftyError),
    #[error("Song not found")]
    SongNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<u64>,
    pub source: String,
}

pub trait MusicSource: Send + Sync {
    fn name(&self) -> &str;
    fn get_stream(&self, song_id: &str) -> Box<dyn Future<Output = Result<AudioInput, SourceError>> + Send>;
    fn get_metadata(&self, song_id: &str) -> Box<dyn Future<Output = Result<Song, SourceError>> + Send>;
    fn get_library(&self) -> Box<dyn Future<Output = Vec<Song>> + Send>;
}

#[derive(Clone)]
pub struct LocalSource {
    music_dir: PathBuf,
    library: Arc<RwLock<HashMap<String, PathBuf>>>, // song_id -> file path
}

impl LocalSource {
    pub async fn new() -> Result<Self, SourceError> {
        let music_dir = PathBuf::from("C:\\Users\\otoya\\Music");
        let library = Arc::new(RwLock::new(HashMap::new()));
        
        let mut local_source = Self {
            music_dir,
            library,
        };
        
        local_source.scan_directory().await?;
        
        Ok(local_source)
    }
    
    async fn scan_directory(&mut self) -> Result<(), SourceError> {
        let mut library_write = self.library.write().await;
        library_write.clear();
        
        self.scan_recursive(&self.music_dir, &mut library_write)?;
        
        Ok(())
    }
    
    fn scan_recursive(&self, dir: &Path, library: &mut HashMap<String, PathBuf>) -> Result<(), SourceError> {
        if dir.is_dir() {
            for entry in read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    self.scan_recursive(&path, library)?;
                } else if self.is_audio_file(&path) {
                    let song_id = self.generate_song_id(&path);
                    library.insert(song_id, path);
                }
            }
        }
        
        Ok(())
    }
    
    fn is_audio_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_str().unwrap_or("");
            matches!(ext_str.to_lowercase().as_str(), "mp3" | "flac" | "m4a" | "ogg")
        } else {
            false
        }
    }
    
    fn generate_song_id(&self, path: &Path) -> String {
        let path_str = path.to_str().unwrap_or("");
        let mut hasher = Sha1::new();
        hasher.update(path_str);
        let hash = hasher.finalize();
        format!("local:{:x}", hash)
    }
    
    async fn get_file_path(&self, song_id: &str) -> Option<PathBuf> {
        let library_read = self.library.read().await;
        library_read.get(song_id).cloned()
    }
    
    fn parse_metadata(&self, path: &Path) -> Result<Song, SourceError> {
        let mut file = File::open(path)?;
        let tagged_file = TaggedFile::read_from(&mut file, ParseOptions::default())?;
        
        let mut title = String::new();
        let mut artist = String::new();
        let mut album = None;
        let mut duration = None;
        
        // Try to get metadata from tags
        if let Some(tag) = tagged_file.primary_tag() {
            if let Some(t) = tag.title() {
                title = t.to_string();
            }
            if let Some(a) = tag.artist() {
                artist = a.to_string();
            }
            if let Some(al) = tag.album() {
                album = Some(al.to_string());
            }
        }
        
        // Get duration
        duration = Some((tagged_file.properties().duration().as_secs_f64() * 1000.0) as u64);
        
        // Fallback to file path if metadata is missing
        if title.is_empty() {
            if let Some(file_name) = path.file_name() {
                if let Some(file_name_str) = file_name.to_str() {
                    title = Self::remove_extension(file_name_str).to_string();
                }
            }
        }
        
        if artist.is_empty() {
            if let Some(parent) = path.parent() {
                if let Some(parent_name) = parent.file_name() {
                    if let Some(parent_name_str) = parent_name.to_str() {
                        artist = parent_name_str.to_string();
                    }
                }
            }
        }
        
        if album.is_none() {
            if let Some(parent) = path.parent() {
                if let Some(grandparent) = parent.parent() {
                    if let Some(grandparent_name) = grandparent.file_name() {
                        if let Some(grandparent_name_str) = grandparent_name.to_str() {
                            album = Some(grandparent_name_str.to_string());
                        }
                    }
                }
            }
        }
        
        let song_id = self.generate_song_id(path);
        
        Ok(Song {
            id: song_id,
            title,
            artist,
            album,
            duration,
            source: "local".to_string(),
        })
    }
    
    fn remove_extension(file_name: &str) -> &str {
        if let Some(dot_idx) = file_name.rfind('.') {
            &file_name[..dot_idx]
        } else {
            file_name
        }
    }
}

impl MusicSource for LocalSource {
    fn name(&self) -> &str {
        "local"
    }
    
    fn get_stream(&self, song_id: &str) -> Box<dyn Future<Output = Result<AudioInput, SourceError>> + Send> {
        let self_clone = self.clone();
        let song_id = song_id.to_string();
        
        Box::new(async move {
            if let Some(path) = self_clone.get_file_path(&song_id).await {
                let file = File::open(path)?;
                Ok(BufReader::new(file))
            } else {
                Err(SourceError::SongNotFound)
            }
        })
    }
    
    fn get_metadata(&self, song_id: &str) -> Box<dyn Future<Output = Result<Song, SourceError>> + Send> {
        let self_clone = self.clone();
        let song_id = song_id.to_string();
        
        Box::new(async move {
            if let Some(path) = self_clone.get_file_path(&song_id).await {
                self_clone.parse_metadata(&path)
            } else {
                Err(SourceError::SongNotFound)
            }
        })
    }
    
    fn get_library(&self) -> Box<dyn Future<Output = Vec<Song>> + Send> {
        let self_clone = self.clone();
        
        Box::new(async move {
            let library_read = self_clone.library.read().await;
            let mut songs = Vec::new();
            
            for path in library_read.values() {
                if let Ok(song) = self_clone.parse_metadata(path) {
                    songs.push(song);
                }
            }
            
            songs
        })
    }
}
