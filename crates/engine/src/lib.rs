use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub type AudioInput = BufReader<File>;

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
    
    pub fn play(&self, input: AudioInput) {
        let mut sink_guard = self.sink.lock().unwrap();
        let mut stream_guard = self._stream.lock().unwrap();
        
        // Stop any existing playback
        if sink_guard.is_some() {
            sink_guard.as_mut().unwrap().stop();
        }
        
        // Create new output stream and sink
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        
        // Decode and play the audio
        let source = Decoder::new(input).unwrap();
        sink.append(source);
        
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
