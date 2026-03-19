# LaetiBeat Server

LaetiBeat Server 是一个高性能的音乐流媒体后端服务，基于 Rust 语言开发，使用 axum + tokio 构建。

## 项目结构

```
laetibeat-server/
├── crates/
│   ├── api/         # HTTP API 层
│   ├── core/        # 核心业务逻辑
│   ├── engine/      # 音频播放引擎
│   ├── server/      # 服务器启动逻辑
│   └── source/      # 音乐源管理
├── API.md           # API 文档
├── Cargo.toml       # 项目配置
└── README.md        # 项目说明
```

## 模块说明

### 1. Core 模块

核心业务逻辑模块，包含：

- **Controller**：核心控制器，处理所有命令和状态管理
- **PlayerState**：播放器状态管理
- **Queue**：播放队列管理
- **Command**：命令系统
- **Event**：事件系统

### 2. Source 模块

音乐源管理模块，包含：

- **AudioStream**：统一的音频流抽象
- **MusicSource**：音乐源接口
- **LocalSource**：本地文件系统音乐源
- **SourceManager**：音乐源管理

### 3. Engine 模块

纯后端状态管理模块：

- **Engine**：状态管理引擎
- **EngineEvent**：引擎事件

### 4. API 模块

HTTP API 层，基于 axum 实现：

- **RESTful API**：提供标准 RESTful 接口
- **WebSocket**：提供实时状态更新
- **Stream Handler**：音频流处理

### 5. Server 模块

服务器启动和配置模块：

- **Main**：服务器入口
- **Router**：路由配置

## 主要功能

1. **音乐库管理**：扫描和管理本地音乐文件
2. **状态管理**：管理播放器状态，包括播放、暂停、停止等状态
3. **队列管理**：添加、移除、清空队列，支持随机播放和循环模式
4. **音频流**：支持 HTTP Range 请求，实现音频流式传输
5. **实时状态**：通过 WebSocket 提供实时播放器状态

## 技术特点

- **高性能**：基于 Rust 和 tokio 的异步处理
- **低内存**：流式处理音频数据，避免一次性加载大文件
- **可扩展**：模块化设计，易于添加新的音乐源
- **标准兼容**：支持 HTTP Range 请求，符合 HTTP 标准
- **类型安全**：利用 Rust 的类型系统确保代码安全

## 支持的音频格式

- MP3 (`audio/mpeg`)
- FLAC (`audio/flac`)
- M4A (`audio/mp4`)
- OGG (`audio/ogg`)
- WAV (`audio/wav`)

## 安装和运行

### 前置条件

- Rust 1.60+ 
- tokio 1.0+
- 音频设备（用于本地播放）

### 安装

```bash
git clone <repository-url>
cd laetibeat-server
cargo build --release
```

### 运行

```bash
cargo run --release
```

服务器默认在 `http://localhost:3000` 运行。

## API 文档

详细的 API 文档请查看 [API.md](API.md) 文件。

## 配置

### 本地音乐目录

默认本地音乐目录为 `C:\Users\otoya\Music`，可在 `LocalSource::new()` 方法中修改。

### 服务器端口

默认服务器端口为 3000，可在 `server/src/main.rs` 中修改。

## 示例使用

### 1. 列出音乐库

```bash
GET http://localhost:3000/api/v2/library
```

### 2. 播放音乐

```bash
POST http://localhost:3000/api/v2/load
Content-Type: application/json

{
  "song_id": "local:C:/Users/otoya/Music/song.mp3"
}

POST http://localhost:3000/api/v2/play
```

### 3. 流式播放

```bash
GET http://localhost:3000/api/v2/stream/local:C:/Users/otoya/Music/song.mp3
```

## 扩展指南

### 添加新的音乐源

1. 实现 `MusicSource` trait
2. 将新的音乐源注册到 `SourceManager`

### 支持新的音频格式

在 `get_mime_type` 函数中添加新的文件扩展名到 MIME 类型的映射。

## 性能优化

1. **音频流处理**：使用 `ReaderStream` 实现零拷贝流式传输
2. **Range 请求**：支持 HTTP Range 请求，实现高效的音频定位
3. **异步处理**：使用 tokio 的异步 IO，避免阻塞
4. **内存管理**：流式处理大文件，避免一次性加载到内存

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！