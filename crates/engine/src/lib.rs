use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

pub type AudioInput = BufReader<File>;

pub struct Engine {
    sink: Arc<Mutex<Option<Sink>>>,
    _stream: Arc<Mutex<Option<OutputStream>>>,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn new() -> Self {
        Self {
            sink: Arc::new(Mutex::new(None)),
            _stream: Arc::new(Mutex::new(None)),
        }
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
