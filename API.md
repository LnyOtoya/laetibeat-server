# LaetiBeat Server API 文档

## 概述

LaetiBeat Server 提供了一套完整的 RESTful API，用于控制音乐播放器和获取音乐库信息。同时，还提供了 WebSocket 接口用于实时获取播放器状态。

## 基础信息

- **Base URL**: `http://localhost:3000`
- **API Version**: v1, v2
- **Content-Type**: `application/json`
- **WebSocket Endpoint**: `ws://localhost:3000/ws/status`

## 响应格式

所有 API 响应都采用统一的格式：

### V1 响应格式

V1 接口直接返回 HTTP 状态码，无 JSON 响应体。

### V2 响应格式

```json
{
  "success": true,
  "data": null,
  "state": {
    "status": "Playing",
    "current": {
      "id": "local:C:/Users/otoya/Music/song.mp3",
      "title": "Song Title",
      "artist": "Artist Name",
      "album": "Album Name",
      "duration": 240000,
      "source": "local"
    },
    "position": 120000,
    "duration": 240000,
    "queue": {
      "tracks": [
        {
          "id": "local:C:/Users/otoya/Music/song1.mp3",
          "title": "Song 1",
          "artist": "Artist 1",
          "album": "Album 1",
          "duration": 240000,
          "source": "local"
        }
      ],
      "current_index": 0,
      "shuffle": false,
      "repeat": "Off",
      "original_order": [
        {
          "id": "local:C:/Users/otoya/Music/song1.mp3",
          "title": "Song 1",
          "artist": "Artist 1",
          "album": "Album 1",
          "duration": 240000,
          "source": "local"
        }
      ]
    }
  },
  "error": null
}
```

## API 端点

### V1 接口（兼容旧版本）

#### 播放控制

| 方法   | 端点                | 描述      | 响应                            |
| ---- | ----------------- | ------- | ----------------------------- |
| POST | `/api/v1/play`    | 开始播放    | 200 OK                        |
| POST | `/api/v1/pause`   | 暂停播放    | 200 OK                        |
| POST | `/api/v1/stop`    | 停止播放    | 200 OK                        |
| POST | `/api/v1/load`    | 加载歌曲    | 200 OK                        |
| GET  | `/api/v1/status`  | 获取播放器状态 | 200 OK (PlayerState JSON)     |
| GET  | `/api/v1/library` | 获取音乐库   | 200 OK (LibraryResponse JSON) |

#### 加载歌曲请求体

```json
{
  "song_id": "local:C:/Users/otoya/Music/song.mp3"
}
```

### V2 接口

#### 播放控制

| 方法   | 端点                | 描述      | 响应                   |
| ---- | ----------------- | ------- | -------------------- |
| POST | `/api/v2/play`    | 开始播放    | 200 OK (ApiResponse) |
| POST | `/api/v2/pause`   | 暂停播放    | 200 OK (ApiResponse) |
| POST | `/api/v2/stop`    | 停止播放    | 200 OK (ApiResponse) |
| POST | `/api/v2/load`    | 加载歌曲    | 200 OK (ApiResponse) |
| POST | `/api/v2/next`    | 下一曲     | 200 OK (ApiResponse) |
| POST | `/api/v2/prev`    | 上一曲     | 200 OK (ApiResponse) |
| GET  | `/api/v2/status`  | 获取播放器状态 | 200 OK (ApiResponse) |
| GET  | `/api/v2/library` | 获取音乐库   | 200 OK (ApiResponse) |

#### 队列管理

| 方法   | 端点                      | 描述         | 响应                   |
| ---- | ----------------------- | ---------- | -------------------- |
| POST | `/api/v2/queue/add`     | 添加歌曲到队列    | 200 OK (ApiResponse) |
| POST | `/api/v2/queue/remove`  | 从队列移除歌曲    | 200 OK (ApiResponse) |
| POST | `/api/v2/queue/clear`   | 清空队列       | 200 OK (ApiResponse) |
| POST | `/api/v2/queue/shuffle` | 设置随机播放     | 200 OK (ApiResponse) |
| POST | `/api/v2/queue/repeat`  | 设置循环模式     | 200 OK (ApiResponse) |
| POST | `/api/v2/queue/play`    | 播放队列中的指定歌曲 | 200 OK (ApiResponse) |

#### 加载歌曲请求体

```json
{
  "song_id": "local:C:/Users/otoya/Music/song.mp3"
}
```

#### 添加到队列请求体

```json
{
  "song_id": "local:C:/Users/otoya/Music/song.mp3"
}
```

#### 从队列移除请求体

```json
{
  "index": 0
}
```

#### 设置随机播放请求体

```json
{
  "enabled": true
}
```

#### 设置循环模式请求体

```json
{
  "mode": "Off" // 可选值: Off, One, All
}
```

#### 播放队列中的指定歌曲请求体

```json
{
  "index": 0
}
```

### 音频流接口

| 方法  | 端点                   | 描述    | 响应                           |
| --- | -------------------- | ----- | ---------------------------- |
| GET | `/api/v2/stream/:id` | 获取音频流 | 200 OK 或 206 Partial Content |

#### 示例

```bash
GET http://localhost:3000/api/v2/stream/local:C:/Users/otoya/Music/song.mp3
```

#### 支持的 Range 请求

```bash
GET http://localhost:3000/api/v2/stream/local:C:/Users/otoya/Music/song.mp3
Range: bytes=0-999
```

## WebSocket 接口

### 状态更新

**端点**: `ws://localhost:3000/ws/status`

#### 连接建立

当连接建立时，服务器会发送当前的播放器状态。

#### 消息格式

服务器会发送 JSON 格式的播放器状态更新：

```json
{
  "status": "Playing",
  "current": {
    "id": "local:C:/Users/otoya/Music/song.mp3",
    "title": "Song Title",
    "artist": "Artist Name",
    "album": "Album Name",
    "duration": 240000,
    "source": "local"
  },
  "position": 120000,
  "duration": 240000,
  "queue": {
    "tracks": [
      {
        "id": "local:C:/Users/otoya/Music/song1.mp3",
        "title": "Song 1",
        "artist": "Artist 1",
        "album": "Album 1",
        "duration": 240000,
        "source": "local"
      }
    ],
    "current_index": 0,
    "shuffle": false,
    "repeat": "Off",
    "original_order": [
      {
        "id": "local:C:/Users/otoya/Music/song1.mp3",
        "title": "Song 1",
        "artist": "Artist 1",
        "album": "Album 1",
        "duration": 240000,
        "source": "local"
      }
    ]
  }
}
```

## 数据结构

### Track

```json
{
  "id": "local:C:/Users/otoya/Music/song.mp3",
  "title": "Song Title",
  "artist": "Artist Name",
  "album": "Album Name",
  "duration": 240000, // 毫秒
  "source": "local"
}
```

### PlayerState

```json
{
  "status": "Playing", // 可选值: Idle, Playing, Paused, Stopped, Ended
  "current": { /* Track 对象 */ },
  "position": 120000, // 当前播放位置（毫秒）
  "duration": 240000, // 总时长（毫秒）
  "queue": {
    "tracks": [ /* Track 对象数组 */ ],
    "current_index": 0,
    "shuffle": false,
    "repeat": "Off", // 可选值: Off, One, All
    "original_order": [ /* Track 对象数组 */ ]
  }
}
```

### ApiResponse

```json
{
  "success": true,
  "data": null, // 或具体数据
  "state": { /* PlayerState 对象 */ },
  "error": null // 或错误对象
}
```

### ApiError

```json
{
  "code": "ERROR_CODE",
  "message": "Error message"
}
```

## 错误处理

| 状态码 | 描述                                 |
| --- | ---------------------------------- |
| 400 | Bad Request - 请求参数错误               |
| 404 | Not Found - 资源不存在                  |
| 416 | Range Not Satisfiable - Range 请求无效 |
| 500 | Internal Server Error - 服务器内部错误    |

## 示例使用

### 1. 获取音乐库

```bash
curl http://localhost:3000/api/v2/library
```

### 2. 加载并播放歌曲

```bash
curl -X POST http://localhost:3000/api/v2/load \
  -H "Content-Type: application/json" \
  -d '{"song_id": "local:C:/Users/otoya/Music/song.mp3"}'

curl -X POST http://localhost:3000/api/v2/play
```

### 3. 控制播放

```bash
# 暂停
curl -X POST http://localhost:3000/api/v2/pause

# 继续播放
curl -X POST http://localhost:3000/api/v2/play

# 停止
curl -X POST http://localhost:3000/api/v2/stop

# 下一曲
curl -X POST http://localhost:3000/api/v2/next

# 上一曲
curl -X POST http://localhost:3000/api/v2/prev
```

### 4. 队列管理

```bash
# 添加歌曲到队列
curl -X POST http://localhost:3000/api/v2/queue/add \
  -H "Content-Type: application/json" \
  -d '{"song_id": "local:C:/Users/otoya/Music/song2.mp3"}'

# 从队列移除歌曲
curl -X POST http://localhost:3000/api/v2/queue/remove \
  -H "Content-Type: application/json" \
  -d '{"index": 0}'

# 清空队列
curl -X POST http://localhost:3000/api/v2/queue/clear

# 开启随机播放
curl -X POST http://localhost:3000/api/v2/queue/shuffle \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'

# 设置循环模式为单曲循环
curl -X POST http://localhost:3000/api/v2/queue/repeat \
  -H "Content-Type: application/json" \
  -d '{"mode": "One"}'

# 播放队列中的第二首歌曲
curl -X POST http://localhost:3000/api/v2/queue/play \
  -H "Content-Type: application/json" \
  -d '{"index": 1}'
```

### 5. 流式播放

```bash
# 完整播放
curl http://localhost:3000/api/v2/stream/local:C:/Users/otoya/Music/song.mp3 \
  -o song.mp3

# 部分播放（Range 请求）
curl http://localhost:3000/api/v2/stream/local:C:/Users/otoya/Music/song.mp3 \
  -H "Range: bytes=0-999" \
  -o partial_song.mp3
```

## 最佳实践

1. **使用 V2 接口**：V2 接口提供了更完整的响应信息和错误处理
2. **使用 WebSocket**：对于需要实时状态更新的应用，使用 WebSocket 可以减少 HTTP 请求
3. **Range 请求**：对于大文件，使用 Range 请求可以实现高效的音频定位
4. **错误处理**：处理 API 响应中的 error 字段，优雅处理错误情况
5. **批量操作**：对于队列管理，合理使用批量操作减少 API 调用

## 注意事项

1. **音频格式**：支持 MP3、FLAC、M4A、OGG、WAV 格式
2. **文件路径**：本地音乐文件路径需要使用绝对路径
3. **性能**：对于大文件，使用流式传输避免内存溢出
4. **兼容性**：V1 接口仅用于向后兼容，建议使用 V2 接口
5. **安全性**：当前实现没有身份验证，建议在生产环境中添加认证机制

