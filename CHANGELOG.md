# Changelog

本项目的版本变更记录。

## 0.1.4 - 2026-05-07

### 新增

- 新增 `ChatGptImageClient`，支持通过 Responses `image_generation` 工具调用 ChatGPT 生图能力。
- 新增 `ChatGptImageRequest`、`ChatGptImageResponse`、`ChatGptReferenceImage` 公共类型。
- 新增独立的 ChatGPT 生图 CLI：
  `cargo run --bin generate-image-via-responses -- ...`
- 新增 `Client::generate_chatgpt_image(...)` 与 `Client::chatgpt_image()` 统一入口。
- 新增 `ClientBuilder::with_chatgpt_image_api_key(...)`，支持聊天模型与生图模型分开配置 key。
- 新增 `GeneratedImage::save_with_metadata(...)`，返回保存后的相对路径、绝对路径和字节大小。
- 新增 `SavedImageInfo` 类型。
- 新增 `format_duration_ms(...)` 工具函数。

### 变更

- `ChatGptImageClient::from_env()` 默认读取 `YLS_CODEX_KEY`。
- `Client::from_env()` / `ClientBuilder::from_env()` 现在同时要求：
  - `YLS_AGI_KEY` 用于常规聊天能力
  - `YLS_CODEX_KEY` 用于 ChatGPT 生图能力
- CLI 固定使用外层模型 `gpt-5.4` 和图片模型 `gpt-image-2`，不再允许通过命令行覆盖模型。
- CLI 输出现在包含：
  - `outputPath`
  - `absoluteOutputPath`
  - `inferredExtension`
  - `byteLength`
  - `totalDurationMs`
  - `totalDurationFormatted`
- `README.md` 整体改为中文说明，并同步补充 ChatGPT 生图、双 key、CLI 输出和保存元数据说明。

### 兼容性

- 保留了 `ResponsesClient`、`ResponsesImageRequest`、`ResponsesImageResponse`、`ResponsesReferenceImage` 等旧命名兼容别名，但已标记为 deprecated。
- 保留了 `generate_image_via_responses(...)`、`responses()`、`with_responses_auth_mode(...)`、`with_responses_base_url(...)` 等旧入口，但已标记为 deprecated。

### 测试

- 补充 ChatGPT 生图请求序列化测试。
- 补充统一客户端路由到 ChatGPT 生图能力的测试。
- 补充独立 ChatGPT 生图 key 覆盖测试。
