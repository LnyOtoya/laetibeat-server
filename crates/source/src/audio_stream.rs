use std::pin::Pin;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, ReadBuf};

/// 统一音频流抽象，用于表示可流式读取的音频数据
pub enum AudioStream {
    /// 本地文件
    File(File),
    /// 通用异步流
    Stream(Pin<Box<dyn AsyncRead + Send + Unpin>>),
}

/// 确保 AudioStream 满足 Send + Unpin 约束
unsafe impl Send for AudioStream {}
impl Unpin for AudioStream {}

/// 手动实现 Debug trait，因为 dyn AsyncRead 没有实现 Debug
impl std::fmt::Debug for AudioStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioStream::File(file) => write!(f, "AudioStream::File({:?})", file),
            AudioStream::Stream(_) => write!(f, "AudioStream::Stream(<async stream>)")
        }
    }
}

/// 定义一个同时实现 AsyncRead 和 AsyncSeek 的 trait
pub trait AsyncReadSeek: AsyncRead + AsyncSeek + Send + Unpin {}

/// 为所有实现了 AsyncRead + AsyncSeek + Send + Unpin 的类型实现 AsyncReadSeek
impl<T: AsyncRead + AsyncSeek + Send + Unpin> AsyncReadSeek for T {}

impl AudioStream {
    /// 将 AudioStream 转换为统一的 AsyncRead 接口
    pub fn into_async_read(self) -> Pin<Box<dyn AsyncRead + Send + Unpin>> {
        match self {
            AudioStream::File(file) => Box::pin(file),
            AudioStream::Stream(stream) => stream,
        }
    }
    
    /// 尝试将 AudioStream 转换为 AsyncReadSeek
    pub fn into_async_seek(self) -> Option<Pin<Box<dyn AsyncReadSeek>>> {
        match self {
            AudioStream::File(file) => Some(Box::pin(file)),
            AudioStream::Stream(_) => None,
        }
    }
}

/// 为 AudioStream 实现 AsyncRead trait
impl AsyncRead for AudioStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            AudioStream::File(file) => Pin::new(file).poll_read(cx, buf),
            AudioStream::Stream(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

/// 为 AudioStream 实现 AsyncSeek trait
impl AsyncSeek for AudioStream {
    fn start_seek(
        mut self: Pin<&mut Self>,
        pos: std::io::SeekFrom,
    ) -> std::io::Result<()> {
        match &mut *self {
            AudioStream::File(file) => Pin::new(file).start_seek(pos),
            AudioStream::Stream(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Stream does not support seeking",
            )),
        }
    }
    
    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<u64>> {
        match &mut *self {
            AudioStream::File(file) => Pin::new(file).poll_complete(cx),
            AudioStream::Stream(_) => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Stream does not support seeking",
            ))),
        }
    }
}

/// 从 File 转换为 AudioStream 的示例
#[cfg(test)]
async fn file_to_audio_stream_example() -> Result<AudioStream, std::io::Error> {
    let file = File::open("path/to/audio.mp3").await?;
    Ok(AudioStream::File(file))
}

/// 使用 AudioStream 的示例
async fn use_audio_stream_example(stream: AudioStream) -> Result<(), std::io::Error> {
    let mut async_read = stream.into_async_read();
    let mut buffer = Vec::new();
    
    // 读取数据
    async_read.read_to_end(&mut buffer).await?;
    
    // 处理数据（例如发送给客户端）
    println!("Read {} bytes of audio data", buffer.len());
    
    Ok(())
}

/// 设计说明：
/// 1. 为什么使用 AsyncRead：
///    - 支持非阻塞 IO，适合 tokio 异步环境
///    - 统一不同数据源的读取接口
///    - 便于与 HTTP 服务器集成，直接返回流数据
///
/// 2. 为什么使用 trait object（dyn）：
///    - 支持动态分发，允许在运行时处理不同类型的流
///    - 提供统一接口，隐藏具体实现细节
///    - 便于未来扩展新的数据源类型
///
/// 3. 如何支持未来扩展（HTTP / 云）：
///    - 对于 HTTP 数据源，可以将 reqwest 的 Response 转换为 AsyncRead
///    - 对于云存储，可以使用相应 SDK 提供的流接口
///    - 只需实现 AsyncRead trait 即可集成到 AudioStream
///
/// 4. 优势：
///    - 统一抽象，简化代码
///    - 异步支持，提高性能
///    - 易于扩展，适应不同数据源
///    - 与 tokio 生态系统良好集成

/// 示例：如何从本地文件创建 AudioStream
pub async fn example_from_file(path: &str) -> Result<AudioStream, std::io::Error> {
    let file = File::open(path).await?;
    Ok(AudioStream::File(file))
}

// 示例：如何从 HTTP 响应创建 AudioStream（伪代码）
// 注意：需要在 Cargo.toml 中添加 reqwest 依赖
// reqwest = { version = "0.11", features = ["stream"] }
/*
#[cfg(feature = "http")]
pub async fn example_from_http(url: &str) -> Result<AudioStream, reqwest::Error> {
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await?;
    
    let stream = response.bytes_stream();
    let async_read = stream.into_async_read();
    
    Ok(AudioStream::Stream(Box::pin(async_read)))
}
*/
