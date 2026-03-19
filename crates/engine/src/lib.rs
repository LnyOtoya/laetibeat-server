use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;

// 从 source 模块导入 AudioStream
use music_backend_source::AudioStream;

#[derive(Debug)]
pub enum EngineEvent {
    PlaybackEnded,
}

pub struct Engine {
    sink: Arc<Mutex<Option<Sink>>>,
    _stream: Arc<Mutex<Option<OutputStream>>>,
    event_tx: Option<mpsc::Sender<EngineEvent>>,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Clone for Engine {
    fn clone(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            _stream: self._stream.clone(),
            event_tx: self.event_tx.clone(),
        }
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            sink: Arc::new(Mutex::new(None)),
            _stream: Arc::new(Mutex::new(None)),
            event_tx: None,
        }
    }
    
    pub fn set_event_sender(&mut self, tx: mpsc::Sender<EngineEvent>) {
        self.event_tx = Some(tx);
    }
    
    pub fn play(&self, input: AudioStream) {
        let mut sink_guard = self.sink.lock().unwrap();
        let mut stream_guard = self._stream.lock().unwrap();
        
        // Stop any existing playback
        if sink_guard.is_some() {
            sink_guard.as_mut().unwrap().stop();
        }
        
        // Create new output stream and sink
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        
        // 根据 AudioStream 类型创建解码器
        match input {
            AudioStream::File(path) => {
                let file = File::open(path).unwrap();
                let reader = BufReader::new(file);
                let source = Decoder::new(reader).unwrap();
                sink.append(source);
            }
            AudioStream::Stream(mut stream) => {
                // 对于 Stream 类型，我们需要将 AsyncRead 转换为同步 Read
                // 这里使用一个简单的方法：读取所有数据到内存中
                let mut buffer = Vec::new();
                // 注意：这里在同步上下文中使用了阻塞的方式读取异步流
                // 在实际生产环境中，应该使用更优雅的方式处理
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        stream.read_to_end(&mut buffer).await.unwrap();
                    });
                
                use std::io::Cursor;
                let cursor = Cursor::new(buffer);
                let source = Decoder::new(cursor).unwrap();
                sink.append(source);
            }
            AudioStream::Bytes(bytes) => {
                use std::io::Cursor;
                let cursor = Cursor::new(bytes);
                let source = Decoder::new(cursor).unwrap();
                sink.append(source);
            }
        }
        
        // Store the sink and stream
        *sink_guard = Some(sink);
        *stream_guard = Some(stream);
        
        // Spawn a task to monitor playback completion
        let sink_clone = self.sink.clone();
        let event_tx_clone = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let is_empty = {
                    let sink_guard = sink_clone.lock().unwrap();
                    match sink_guard.as_ref() {
                        Some(sink) => sink.empty(),
                        None => true,
                    }
                };
                
                if is_empty {
                    if let Some(tx) = &event_tx_clone {
                        let _ = tx.send(EngineEvent::PlaybackEnded).await;
                    }
                    break;
                }
            }
        });
    }
    
    pub fn pause(&self) {
        let mut sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_mut() {
            sink.pause();
        }
    }
    
    pub fn stop(&self) {
        let mut sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_mut() {
            sink.stop();
        }
    }
}