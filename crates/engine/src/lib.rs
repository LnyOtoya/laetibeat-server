use tokio::sync::mpsc;

// 从 source 模块导入 AudioStream
use music_backend_source::AudioStream;

#[derive(Debug)]
pub enum EngineEvent {
    PlaybackEnded,
}

pub struct Engine {
    event_tx: Option<mpsc::Sender<EngineEvent>>,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Clone for Engine {
    fn clone(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
        }
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            event_tx: None,
        }
    }
    
    pub fn set_event_sender(&mut self, tx: mpsc::Sender<EngineEvent>) {
        self.event_tx = Some(tx);
    }
    
    pub fn play(&self, _input: AudioStream) {
        // 纯后端模式：不执行实际播放，只更新状态
    }
    
    pub fn pause(&self) {
        // 纯后端模式：不执行实际暂停，只更新状态
    }
    
    pub fn stop(&self) {
        // 纯后端模式：不执行实际停止，只更新状态
    }
}