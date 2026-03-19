use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use lofty::{AudioFile, TaggedFile, TaggedFileExt, LoftyError, Accessor, ParseOptions};

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Metadata error: {0}")]
    MetadataError(#[from] LoftyError),
    #[error("Track not found")]
    TrackNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,      // 格式：source:unique_id，例如 local:C:/xxx.mp3
    pub title: String,
    pub artist: String,
    pub album: String,    // 不再使用 Option，确保总是有值
    pub duration: u64,    // 不再使用 Option，确保总是有值
    pub source: String,   // 如 "local"
}

// 音频流抽象
pub enum AudioStream {
    File(PathBuf),                                 // 本地文件路径
    Stream(Pin<Box<dyn Read + Send + Unpin>>),     // 通用流
    Bytes(Vec<u8>),                                // 内存中的字节
}

impl std::fmt::Debug for AudioStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioStream::File(path) => write!(f, "AudioStream::File({:?})", path),
            AudioStream::Stream(_) => write!(f, "AudioStream::Stream(<stream>)",),
            AudioStream::Bytes(bytes) => write!(f, "AudioStream::Bytes({} bytes)", bytes.len()),
        }
    }
}

pub trait MusicSource: Send + Sync {
    fn name(&self) -> &str;
    fn get_track(&self, id: &str) -> Pin<Box<dyn Future<Output = Result<Track, SourceError>> + Send + '_>>;
    fn get_stream(&self, id: &str) -> Pin<Box<dyn Future<Output = Result<AudioStream, SourceError>> + Send + '_>>;
    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Track>, SourceError>> + Send + '_>>;
    fn search(&self, keyword: &str) -> Pin<Box<dyn Future<Output = Result<Vec<Track>, SourceError>> + Send + '_>> {
        // 默认实现：返回空列表
        Box::pin(async move {
            Ok(Vec::new())
        })
    }
}

#[derive(Clone)]
pub struct LocalSource {
    music_dir: PathBuf,
    library: Arc<RwLock<HashMap<String, PathBuf>>>, // track_id -> file path
}

impl LocalSource {
    pub async fn new() -> Result<Self, SourceError> {
        let music_dir = PathBuf::from(r"C:\Users\otoya\Music");
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
                    let track_id = self.generate_track_id(&path);
                    library.insert(track_id, path);
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
    
    fn generate_track_id(&self, path: &Path) -> String {
        // 使用绝对路径作为唯一标识，格式：local:绝对路径
        if let Some(path_str) = path.to_str() {
            format!("local:{}", path_str)
        } else {
            format!("local:{:?}", path)
        }
    }
    
    async fn get_file_path(&self, track_id: &str) -> Option<PathBuf> {
        let library_read = self.library.read().await;
        library_read.get(track_id).cloned()
    }
    
    fn parse_metadata(&self, path: &Path) -> Result<Track, SourceError> {
        let mut file = File::open(path)?;
        let tagged_file = TaggedFile::read_from(&mut file, ParseOptions::default())?;
        
        let mut title = String::new();
        let mut artist = String::new();
        let mut album = String::new();
        let mut duration = 0u64;
        
        // Try to get metadata from tags
        if let Some(tag) = tagged_file.primary_tag() {
            if let Some(t) = tag.title() {
                title = t.to_string();
            }
            if let Some(a) = tag.artist() {
                artist = a.to_string();
            }
            if let Some(al) = tag.album() {
                album = al.to_string();
            }
        }
        
        // Get duration
        duration = (tagged_file.properties().duration().as_secs_f64() * 1000.0) as u64;
        
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
        
        if album.is_empty() {
            if let Some(parent) = path.parent() {
                if let Some(grandparent) = parent.parent() {
                    if let Some(grandparent_name) = grandparent.file_name() {
                        if let Some(grandparent_name_str) = grandparent_name.to_str() {
                            album = grandparent_name_str.to_string();
                        }
                    }
                }
            }
        }
        
        let track_id = self.generate_track_id(path);
        
        Ok(Track {
            id: track_id,
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
    
    fn get_track(&self, id: &str) -> Pin<Box<dyn Future<Output = Result<Track, SourceError>> + Send + '_>> {
        let self_clone = self.clone();
        let id = id.to_string();
        
        Box::pin(async move {
            if let Some(path) = self_clone.get_file_path(&id).await {
                self_clone.parse_metadata(&path)
            } else {
                Err(SourceError::TrackNotFound)
            }
        })
    }
    
    fn get_stream(&self, id: &str) -> Pin<Box<dyn Future<Output = Result<AudioStream, SourceError>> + Send + '_>> {
        let self_clone = self.clone();
        let id = id.to_string();
        
        Box::pin(async move {
            if let Some(path) = self_clone.get_file_path(&id).await {
                Ok(AudioStream::File(path))
            } else {
                Err(SourceError::TrackNotFound)
            }
        })
    }
    
    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Track>, SourceError>> + Send + '_>> {
        let self_clone = self.clone();
        
        Box::pin(async move {
            let library_read = self_clone.library.read().await;
            let mut tracks = Vec::new();
            
            for path in library_read.values() {
                if let Ok(track) = self_clone.parse_metadata(path) {
                    tracks.push(track);
                }
            }
            
            Ok(tracks)
        })
    }
    
    fn search(&self, keyword: &str) -> Pin<Box<dyn Future<Output = Result<Vec<Track>, SourceError>> + Send + '_>> {
        let self_clone = self.clone();
        let keyword = keyword.to_string();
        
        Box::pin(async move {
            let library_read = self_clone.library.read().await;
            let mut tracks = Vec::new();
            let keyword_lower = keyword.to_lowercase();
            
            for path in library_read.values() {
                if let Ok(track) = self_clone.parse_metadata(path) {
                    if track.title.to_lowercase().contains(&keyword_lower) ||
                       track.artist.to_lowercase().contains(&keyword_lower) ||
                       track.album.to_lowercase().contains(&keyword_lower) {
                        tracks.push(track);
                    }
                }
            }
            
            Ok(tracks)
        })
    }
}

// SourceManager 实现
#[derive(Clone)]
pub struct SourceManager {
    sources: Arc<HashMap<String, Box<dyn MusicSource>>>,
}

impl SourceManager {
    pub fn new(sources: Vec<Box<dyn MusicSource>>) -> Self {
        let mut source_map = HashMap::new();
        for source in sources {
            source_map.insert(source.name().to_string(), source);
        }
        Self {
            sources: Arc::new(source_map),
        }
    }
    
    pub async fn get_track(&self, id: &str) -> Result<Track, SourceError> {
        if let Some((source_name, _)) = self.parse_track_id(id) {
            if let Some(source) = self.sources.get(&source_name) {
                source.get_track(id).await
            } else {
                Err(SourceError::TrackNotFound)
            }
        } else {
            Err(SourceError::TrackNotFound)
        }
    }
    
    pub async fn get_stream(&self, id: &str) -> Result<AudioStream, SourceError> {
        if let Some((source_name, _)) = self.parse_track_id(id) {
            if let Some(source) = self.sources.get(&source_name) {
                source.get_stream(id).await
            } else {
                Err(SourceError::TrackNotFound)
            }
        } else {
            Err(SourceError::TrackNotFound)
        }
    }
    
    pub async fn list(&self) -> Result<Vec<Track>, SourceError> {
        let mut all_tracks = Vec::new();
        for source in self.sources.values() {
            let tracks = source.list().await?;
            all_tracks.extend(tracks);
        }
        Ok(all_tracks)
    }
    
    pub async fn search(&self, keyword: &str) -> Result<Vec<Track>, SourceError> {
        let mut all_tracks = Vec::new();
        for source in self.sources.values() {
            let tracks = source.search(keyword).await?;
            all_tracks.extend(tracks);
        }
        Ok(all_tracks)
    }
    
    fn parse_track_id(&self, id: &str) -> Option<(String, String)> {
        if let Some((source, track_id)) = id.split_once(':') {
            Some((source.to_string(), track_id.to_string()))
        } else {
            None
        }
    }
}