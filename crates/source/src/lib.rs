use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use lofty::{AudioFile, TaggedFile, TaggedFileExt, LoftyError, Accessor, ParseOptions};
use async_trait::async_trait;

// 导入新的 AudioStream 定义
mod audio_stream;
pub use audio_stream::AudioStream;

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

use std::pin::Pin;

#[async_trait]
pub trait MusicSource: Send + Sync {
    fn name(&self) -> &str;
    async fn get_track(&self, id: &str) -> Result<Track, SourceError>;
    async fn get_stream(&self, id: &str) -> Result<AudioStream, SourceError>;
    async fn list(&self) -> Result<Vec<Track>, SourceError>;
    async fn search(&self, keyword: &str) -> Result<Vec<Track>, SourceError> {
        // 默认实现：返回空列表
        Ok(Vec::new())
    }
}

#[derive(Clone)]
pub struct LocalSource {
    music_dir: PathBuf,
    library: Arc<RwLock<HashMap<String, Track>>>, // track_id -> Track
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
    
    fn scan_recursive(&self, dir: &Path, library: &mut HashMap<String, Track>) -> Result<(), SourceError> {
        if dir.is_dir() {
            for entry in read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    if let Err(e) = self.scan_recursive(&path, library) {
                        eprintln!("Error scanning directory {:?}: {:?}", path, e);
                        // 继续扫描其他目录，不中断
                    }
                } else if self.is_audio_file(&path) {
                    if let Ok(track) = self.parse_metadata(&path) {
                        library.insert(track.id.clone(), track);
                    } else {
                        eprintln!("Error parsing metadata for {:?}", path);
                        // 跳过错误文件，继续扫描
                    }
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
    
    async fn get_track_from_cache(&self, track_id: &str) -> Option<Track> {
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

#[async_trait]
impl MusicSource for LocalSource {
    fn name(&self) -> &str {
        "local"
    }
    
    async fn get_track(&self, id: &str) -> Result<Track, SourceError> {
        if let Some(track) = self.get_track_from_cache(id).await {
            Ok(track)
        } else {
            Err(SourceError::TrackNotFound)
        }
    }
    
    async fn get_stream(&self, id: &str) -> Result<AudioStream, SourceError> {
        if let Some(track) = self.get_track_from_cache(id).await {
            // 从 track.id 中提取路径
            if let Some((_, path_str)) = track.id.split_once(':') {
                let path = PathBuf::from(path_str);
                let file = tokio::fs::File::open(path).await?;
                Ok(AudioStream::File(file))
            } else {
                Err(SourceError::TrackNotFound)
            }
        } else {
            Err(SourceError::TrackNotFound)
        }
    }
    
    async fn list(&self) -> Result<Vec<Track>, SourceError> {
        let library_read = self.library.read().await;
        Ok(library_read.values().cloned().collect())
    }
    
    async fn search(&self, keyword: &str) -> Result<Vec<Track>, SourceError> {
        let library_read = self.library.read().await;
        let mut tracks = Vec::new();
        let keyword_lower = keyword.to_lowercase();
        
        for track in library_read.values() {
            if track.title.to_lowercase().contains(&keyword_lower) ||
               track.artist.to_lowercase().contains(&keyword_lower) ||
               track.album.to_lowercase().contains(&keyword_lower) {
                tracks.push(track.clone());
            }
        }
        
        Ok(tracks)
    }
}

// SourceManager 实现
#[derive(Clone)]
pub struct SourceManager {
    sources: Arc<RwLock<HashMap<String, Arc<dyn MusicSource>>>>,
}

impl SourceManager {
    pub fn new(sources: Vec<Arc<dyn MusicSource>>) -> Self {
        let mut source_map = HashMap::new();
        for source in sources {
            source_map.insert(source.name().to_string(), source);
        }
        Self {
            sources: Arc::new(RwLock::new(source_map)),
        }
    }
    
    pub async fn get_track(&self, id: &str) -> Result<Track, SourceError> {
        if let Some((source_name, _)) = self.parse_track_id(id) {
            let sources_read = self.sources.read().await;
            if let Some(source) = sources_read.get(&source_name) {
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
            let sources_read = self.sources.read().await;
            if let Some(source) = sources_read.get(&source_name) {
                source.get_stream(id).await
            } else {
                Err(SourceError::TrackNotFound)
            }
        } else {
            Err(SourceError::TrackNotFound)
        }
    }
    
    pub async fn list(&self) -> Result<Vec<Track>, SourceError> {
        let sources_read = self.sources.read().await;
        let mut all_tracks = Vec::new();
        for source in sources_read.values() {
            if let Ok(tracks) = source.list().await {
                all_tracks.extend(tracks);
            }
        }
        Ok(all_tracks)
    }
    
    pub async fn search(&self, keyword: &str) -> Result<Vec<Track>, SourceError> {
        let sources_read = self.sources.read().await;
        let mut all_tracks = Vec::new();
        for source in sources_read.values() {
            if let Ok(tracks) = source.search(keyword).await {
                all_tracks.extend(tracks);
            }
        }
        Ok(all_tracks)
    }
    
    // 动态注册音源
    pub async fn register_source(&self, source: Arc<dyn MusicSource>) {
        let mut sources_write = self.sources.write().await;
        sources_write.insert(source.name().to_string(), source);
    }
    
    // 移除音源
    pub async fn remove_source(&self, source_name: &str) {
        let mut sources_write = self.sources.write().await;
        sources_write.remove(source_name);
    }
    
    fn parse_track_id(&self, id: &str) -> Option<(String, String)> {
        if let Some((source, track_id)) = id.split_once(':') {
            Some((source.to_string(), track_id.to_string()))
        } else {
            None
        }
    }
}