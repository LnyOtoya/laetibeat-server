# 音乐播放后端核心系统 API 测试文档

## 服务器信息
- **地址**: http://127.0.0.1:3000
- **基础路径**: /api/v1

## API 端点

### 1. 获取音乐库
- **方法**: GET
- **路径**: /api/v1/library
- **描述**: 获取本地音乐库中的所有歌曲
- **响应**: 歌曲列表

**示例请求**:
```bash
curl http://127.0.0.1:3000/api/v1/library
```

**示例响应**:
```json
[
  {
    "id": "local:3085416ba6f168abe46a4add5778b02a7034cabb",
    "title": "下雨天",
    "artist": "南拳妈妈"
  },
  {
    "id": "local:a90a38a6a94e9425b81adbd1019831fd8d057489",
    "title": "牡丹江",
    "artist": "南拳妈妈"
  },
  {
    "id": "local:f9ed5284b0128e617b334d71aec631630341daa4",
    "title": "越长大越孤单",
    "artist": "牛奶咖啡"
  }
]
```

### 2. 加载歌曲
- **方法**: POST
- **路径**: /api/v1/load
- **描述**: 加载指定歌曲到播放器
- **请求体**: JSON格式，包含song_id字段

**示例请求**:
```bash
curl -X POST http://127.0.0.1:3000/api/v1/load \
  -H "Content-Type: application/json" \
  -d '{"song_id":"local:3085416ba6f168abe46a4add5778b02a7034cabb"}'
```

**响应**: 200 OK

### 3. 播放音乐
- **方法**: POST
- **路径**: /api/v1/play
- **描述**: 开始播放当前加载的歌曲

**示例请求**:
```bash
curl -X POST http://127.0.0.1:3000/api/v1/play
```

**响应**: 200 OK

### 4. 暂停音乐
- **方法**: POST
- **路径**: /api/v1/pause
- **描述**: 暂停当前播放的歌曲

**示例请求**:
```bash
curl -X POST http://127.0.0.1:3000/api/v1/pause
```

**响应**: 200 OK

### 5. 停止音乐
- **方法**: POST
- **路径**: /api/v1/stop
- **描述**: 停止当前播放的歌曲

**示例请求**:
```bash
curl -X POST http://127.0.0.1:3000/api/v1/stop
```

**响应**: 200 OK

### 6. 获取状态
- **方法**: GET
- **路径**: /api/v1/status
- **描述**: 获取播放器当前状态
- **响应**: 播放器状态信息

**示例请求**:
```bash
curl http://127.0.0.1:3000/api/v1/status
```

**示例响应**:
```json
{
  "status": "Playing",
  "current": {
    "id": "local:3085416ba6f168abe46a4add5778b02a7034cabb",
    "title": "下雨天",
    "artist": "南拳妈妈",
    "album": "优の良曲 南搞小孩",
    "duration": 253053,
    "source": "local"
  },
  "position": 0,
  "duration": 0
}
```

## 状态说明
- **Idle**: 空闲状态，未加载歌曲
- **Stopped**: 已加载歌曲但未播放
- **Playing**: 正在播放
- **Paused**: 已暂停

## 完整测试流程

1. **获取音乐库**：获取所有可用歌曲
2. **加载歌曲**：选择一首歌曲加载
3. **播放歌曲**：开始播放
4. **获取状态**：确认歌曲正在播放
5. **暂停歌曲**：暂停播放
6. **获取状态**：确认歌曲已暂停
7. **停止歌曲**：停止播放
8. **获取状态**：确认歌曲已停止

## 技术说明
- 所有API响应均为JSON格式
- 错误处理：服务器会返回适当的HTTP状态码
- 本地音乐目录：C:\Users\otoya\Music
- 支持的格式：mp3、flac、m4a、ogg
