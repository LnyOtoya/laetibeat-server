# 音乐播放器后端 API 文档

## 概述

本文档描述了音乐播放器后端的 API 接口，包括 V1 和 V2 版本的接口。V1 接口保持向后兼容，V2 接口提供了更丰富的功能和统一的响应格式。

## 基础信息

- **API 基础路径**: `/api`
- **WebSocket 路径**: `/ws/status`
- **响应格式**: V2 接口使用统一的 JSON 响应格式

## 统一响应结构 (V2)

```json
{
  "success": true,
  "data": null,
  "state": {
    "current": null,
    "status": "idle",
    "position": 0,
    "duration": 0,
    "queue": {
      "tracks": [],
      "original_order": [],
      "current_index": null,
      "shuffle": false,
      "repeat": "off"
    }
  },
  "error": null
}
```

### 响应字段说明

- `success`: 布尔值，表示请求是否成功
- `data`: 可选，包含请求的具体数据
- `state`: 播放器当前状态
- `error`: 可选，包含错误信息

## V1 接口

### 播放控制

| 接口 | 方法 | 描述 | 响应 |
|------|------|------|------|
| `/api/v1/play` | POST | 开始播放 | 200 OK |
| `/api/v1/pause` | POST | 暂停播放 | 200 OK |
| `/api/v1/stop` | POST | 停止播放 | 200 OK |
| `/api/v1/load` | POST | 加载歌曲 | 200 OK |

### 数据获取

| 接口 | 方法 | 描述 | 响应 |
|------|------|------|------|
| `/api/v1/status` | GET | 获取播放器状态 | PlayerState JSON |
| `/api/v1/library` | GET | 获取音乐库 | 歌曲列表 JSON |

## V2 接口

### 播放控制

| 接口 | 方法 | 描述 | 请求体 | 响应 |
|------|------|------|--------|------|
| `/api/v2/play` | POST | 开始播放 | N/A | 统一响应格式 |
| `/api/v2/pause` | POST | 暂停播放 | N/A | 统一响应格式 |
| `/api/v2/stop` | POST | 停止播放 | N/A | 统一响应格式 |
| `/api/v2/load` | POST | 加载歌曲 | `{"song_id": "string"}` | 统一响应格式 |
| `/api/v2/next` | POST | 下一首 | N/A | 统一响应格式 |
| `/api/v2/prev` | POST | 上一首 | N/A | 统一响应格式 |

### 队列管理

| 接口 | 方法 | 描述 | 请求体 | 响应 |
|------|------|------|--------|------|
| `/api/v2/queue/add` | POST | 添加到队列 | `{"song_id": "string"}` | 统一响应格式 |
| `/api/v2/queue/remove` | POST | 从队列移除 | `{"index": 0}` | 统一响应格式 |
| `/api/v2/queue/clear` | POST | 清空队列 | N/A | 统一响应格式 |
| `/api/v2/queue/shuffle` | POST | 设置随机播放 | `{"enabled": true}` | 统一响应格式 |
| `/api/v2/queue/repeat` | POST | 设置重复模式 | `{"mode": "off"}` | 统一响应格式 |
| `/api/v2/queue/play` | POST | 播放队列中的指定歌曲 | `{"index": 0}` | 统一响应格式 |

### 数据获取

| 接口 | 方法 | 描述 | 响应 |
|------|------|------|------|
| `/api/v2/status` | GET | 获取播放器状态 | 统一响应格式 |
| `/api/v2/library` | GET | 获取音乐库 | 统一响应格式 |

## WebSocket 接口

### `/ws/status`

**描述**: 实时获取播放器状态更新

**连接方式**: `ws://localhost:3000/ws/status`

**消息格式**:
- 服务器发送: JSON 格式的 PlayerState
- 客户端发送: 支持 Ping/Pong 消息

**使用示例**:

```javascript
const socket = new WebSocket('ws://localhost:3000/ws/status');

socket.onopen = () => {
  console.log('WebSocket connected');
};

socket.onmessage = (event) => {
  const state = JSON.parse(event.data);
  console.log('Player state updated:', state);
};

socket.onclose = () => {
  console.log('WebSocket disconnected');
};
```

## 数据模型

### PlayerState

```json
{
  "current": {
    "id": "string",
    "title": "string",
    "artist": "string",
    "album": "string",
    "duration": 0,
    "path": "string"
  },
  "status": "idle",
  "position": 0,
  "duration": 0,
  "queue": {
    "tracks": [],
    "original_order": [],
    "current_index": null,
    "shuffle": false,
    "repeat": "off"
  }
}
```

### Song

```json
{
  "id": "string",
  "title": "string",
  "artist": "string",
  "album": "string",
  "duration": 0,
  "path": "string"
}
```

### RepeatMode

- `off`: 不重复
- `one`: 单曲循环
- `all`: 全部循环

### PlaybackStatus

- `idle`: 空闲
- `playing`: 播放中
- `paused`: 暂停
- `stopped`: 停止
- `ended`: 结束

## 错误码

| 错误码 | 描述 |
|--------|------|
| `PLAY_ERROR` | 播放失败 |
| `PAUSE_ERROR` | 暂停失败 |
| `STOP_ERROR` | 停止失败 |
| `LOAD_ERROR` | 加载失败 |
| `NEXT_ERROR` | 下一首失败 |
| `PREV_ERROR` | 上一首失败 |
| `ADD_TO_QUEUE_ERROR` | 添加到队列失败 |
| `REMOVE_FROM_QUEUE_ERROR` | 从队列移除失败 |
| `CLEAR_QUEUE_ERROR` | 清空队列失败 |
| `SET_SHUFFLE_ERROR` | 设置随机播放失败 |
| `SET_REPEAT_ERROR` | 设置重复模式失败 |
| `PLAY_AT_INDEX_ERROR` | 播放队列中的指定歌曲失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 使用示例

### 加载并播放歌曲

```bash
# 加载歌曲
curl -X POST http://localhost:3000/api/v2/load \
  -H "Content-Type: application/json" \
  -d '{"song_id": "local:path/to/song.mp3"}'

# 开始播放
curl -X POST http://localhost:3000/api/v2/play
```

### 管理队列

```bash
# 添加到队列
curl -X POST http://localhost:3000/api/v2/queue/add \
  -H "Content-Type: application/json" \
  -d '{"song_id": "local:path/to/another_song.mp3"}'

# 设置随机播放
curl -X POST http://localhost:3000/api/v2/queue/shuffle \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'

# 设置重复模式
curl -X POST http://localhost:3000/api/v2/queue/repeat \
  -H "Content-Type: application/json" \
  -d '{"mode": "all"}'
```

### 获取状态

```bash
# 获取当前状态
curl http://localhost:3000/api/v2/status

# 获取音乐库
curl http://localhost:3000/api/v2/library
```

## 注意事项

1. 所有 V2 接口都返回统一的响应格式，包含 `success`、`data`、`state` 和 `error` 字段
2. 歌曲 ID 格式为 `source:path`，例如 `local:path/to/song.mp3`
3. WebSocket 连接会自动发送初始状态，并在状态变化时推送更新
4. 错误响应会包含具体的错误信息，便于调试

## 版本差异

| 特性 | V1 | V2 |
|------|----|----|
| 响应格式 | 简单状态码/直接数据 | 统一 JSON 格式 |
| 错误处理 | 简单状态码 | 详细错误信息 |
| 队列管理 | 不支持 | 完整支持 |
| WebSocket | 不支持 | 支持 |
| 重复模式 | 不支持 | 支持 |
| 随机播放 | 不支持 | 支持 |
