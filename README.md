# yls-agi-rust-sdk

面向伊莉思 API 网关的 Rust SDK，提供统一异步 `Client` 和各 Provider 专用客户端。

## 功能特性

- 统一的 `Client`，可向 OpenAI、Gemini、Claude 发起聊天请求
- 在需要更底层控制时，可直接使用各 Provider 专用客户端
- 基于 Tokio 的异步 API
- 支持 OpenAI、Gemini、Claude 的流式输出
- 支持 OpenAI、Gemini、Claude 的多模态图像输入
- 支持 Gemini 生图，以及基于参考图的图片编辑
- 支持基于 Responses `image_generation` 工具的 ChatGPT 生图能力
- 支持可切换的鉴权模式，以适配不同网关部署差异
- `GeneratedImage` 同时提供 `save()` 与 `save_with_metadata()` 保存辅助方法

## 安装

```toml
[dependencies]
yls-agi-rust-sdk = "0.1.3"
```

## 快速开始

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, Client, Provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let request = ChatRequest::new(
        "gpt-4.1",
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("请用中文打个招呼。"),
        ],
    );

    let response = client.chat(Provider::OpenAi, request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

## OpenAI 示例

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, OpenAiClient, OpenAiModel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAiClient::from_env()?;
    let request = ChatRequest::new(
        OpenAiModel::Gpt41,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("用一句话介绍 Rust。"),
        ],
    );

    let response = client.chat(request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

## Gemini 示例

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, GeminiClient, GeminiModel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiClient::from_env()?;
    let request = ChatRequest::new(
        GeminiModel::Gemini3FlashPreview,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("用一句话解释 Rust 的所有权。"),
        ],
    );

    let response = client.chat(request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

如果你的本地代理会导致 Rust `reqwest` 请求异常，可以在构建统一客户端时禁用代理：

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, ClientBuilder, GeminiModel, Provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::from_env()?
        .without_proxy()
        .build()?;

    let request = ChatRequest::new(
        GeminiModel::Gemini3ProPreview,
        vec![ChatMessage::user("用一句话介绍伊莉思")],
    );

    let response = client.chat(Provider::Gemini, request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

## 图像理解示例

```rust
use std::fs;
use yls_agi_rust_sdk::{
    ChatMessage, ChatRequest, ClientBuilder, GeminiModel, GenerationOptions, ImageMimeType,
    Provider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_bytes = fs::read("gemini-image-1.png")?;
    let client = ClientBuilder::from_env()?
        .without_proxy()
        .build()?;

    let request = ChatRequest::new(
        GeminiModel::Gemini3ProPreview,
        vec![
            ChatMessage::system("You are a concise vision assistant."),
            ChatMessage::user("这是什么？请用中文简短描述。")
                .with_image_bytes(ImageMimeType::Png, &image_bytes),
        ],
    )
    .with_options(GenerationOptions {
        temperature: Some(0.2),
        max_tokens: Some(256),
        ..Default::default()
    });

    let response = client.chat(Provider::Gemini, request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

也可以直接传 base64 数据：

```rust
use yls_agi_rust_sdk::{ChatMessage, ImageMimeType};

let message = ChatMessage::user("描述这张图片")
    .with_image_base64(ImageMimeType::Png, image_base64);
```

常用图片 MIME 枚举包括：

- `ImageMimeType::Png`
- `ImageMimeType::Jpeg`
- `ImageMimeType::Webp`
- `ImageMimeType::Gif`
- `ImageMimeType::Bmp`
- `ImageMimeType::Tiff`

如果你需要自定义类型，仍然可以直接传字符串：

```rust
let message = ChatMessage::user("描述这张图片")
    .with_image_bytes("image/heic", &image_bytes);
```

## Gemini 生图示例

```rust
use yls_agi_rust_sdk::{
    GeminiClient, GeminiImageRequest, GeminiModel, GenerationOptions,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiClient::from_env()?;
    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini25FlashImage,
                "生成一只戴着墨镜的写实风橘猫。",
            )
            .with_options(GenerationOptions {
                max_tokens: Some(8192),
                temperature: Some(1.0),
                ..Default::default()
            }),
        )
        .await?;

    if let Some(image) = response.images.first() {
        image.save("gemini-cat.png")?;
    }

    Ok(())
}
```

## Gemini 参考图编辑示例

Gemini 生图模型也支持参考图输入。SDK 会把它序列化为 Gemini 的 `inlineData`，与官方的图片编辑流程一致。

```rust
use std::fs;
use yls_agi_rust_sdk::{
    GeminiClient, GeminiImageRequest, GeminiModel, GenerationOptions, ImageMimeType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_bytes = fs::read("gemini-image-1.png")?;
    let client = GeminiClient::from_env()?;

    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini3ProImagePreview,
                "把这张角色图重绘成像素风游戏角色立绘，保留主体造型特征，透明背景。",
            )
            .with_reference_image_bytes(ImageMimeType::Png, &image_bytes)
            .with_options(GenerationOptions {
                temperature: Some(0.8),
                max_tokens: Some(8192),
                ..Default::default()
            }),
        )
        .await?;

    if let Some(image) = response.images.first() {
        image.save("gemini-edited.png")?;
    }

    Ok(())
}
```

如果需要，也可以附加多张参考图：

```rust
use yls_agi_rust_sdk::{GeminiImageRequest, GeminiModel, ImageMimeType};

let request = GeminiImageRequest::new(
    GeminiModel::Gemini3ProImagePreview,
    "基于这些参考图生成统一风格的角色海报",
)
.with_reference_image_bytes(ImageMimeType::Png, &front_view_bytes)
.with_reference_image_bytes(ImageMimeType::Png, &side_view_bytes);
```

## ChatGPT 生图示例

SDK 现在内置了 `ChatGptImageClient`，用于调用 Responses `image_generation` 工具背后的 ChatGPT 生图能力。

库内调用示例：

```rust
use yls_agi_rust_sdk::{ChatGptImageClient, ChatGptImageRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatGptImageClient::from_env()?;
    let mut response = client
        .generate_image(
            ChatGptImageRequest::new(
                "gpt-5.4",
                "一个打磨完整的 2D 塔防炮台，透明背景。",
            ),
        )
        .await?;

    let saved = response.image.save_with_metadata("output/turret.png")?;
    println!("{}", saved.absolute_output_path);
    Ok(())
}
```

`ChatGptImageClient::from_env()` 默认读取 `YLS_CODEX_KEY`。

如果你需要格式化耗时字符串，也可以直接复用 SDK 导出的工具函数：

```rust
use yls_agi_rust_sdk::format_duration_ms;

println!("{}", format_duration_ms(8421)); // 8.42 s
```

也可以附加参考图 URL 或文件 ID：

```rust
use yls_agi_rust_sdk::{ChatGptImageRequest, ChatGptReferenceImage};

let request = ChatGptImageRequest::new("gpt-5.4", "把这张草图转成精致的游戏图标")
    .with_reference(ChatGptReferenceImage::url("https://example.com/sketch.png"))
    .with_reference_file_id("file-123");
```

也可以写成中文提示词：

```rust
use yls_agi_rust_sdk::{ChatGptImageRequest, ChatGptReferenceImage};

let request = ChatGptImageRequest::new("gpt-5.4", "把这张草图转成精致的游戏图标")
    .with_reference(ChatGptReferenceImage::url("https://example.com/sketch.png"))
    .with_reference_file_id("file-123");
```

自带的 CLI 比 SDK API 更严格：它固定使用外层模型 `gpt-5.4` 和图片模型 `gpt-image-2`。

CLI 用法：

```bash
cargo run --bin generate-image-via-responses -- \
  --prompt "一个打磨完整的 2D 塔防炮台，透明背景" \
  --output output/turret \
  --api-key "$YLS_CODEX_KEY"
```

CLI 支持重复传入 `--reference`、自定义 `--base-url`、以及 `--tool-json`。模型在脚本内部固定，不能通过命令行覆盖。

当前 CLI 的返回 JSON 包含：

- `outputPath`
- `absoluteOutputPath`
- `inferredExtension`
- `byteLength`
- `totalDurationMs`
- `totalDurationFormatted`

## Claude 示例

```rust
use yls_agi_rust_sdk::{
    ChatMessage, ChatRequest, ClaudeClient, ClaudeModel, GenerationOptions,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClaudeClient::from_env()?;
    let request = ChatRequest::new(
        ClaudeModel::ClaudeSonnet4520250929,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("用一句话介绍 Tokio。"),
        ],
    )
    .with_options(GenerationOptions {
        max_tokens: Some(256),
        temperature: Some(0.2),
        ..Default::default()
    });

    let response = client.chat(request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
```

## 流式输出示例

```rust
use futures::StreamExt;
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, Client, Provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let request = ChatRequest::new(
        "gpt-4.1",
        vec![ChatMessage::user("写一段简短的问候语。")],
    )
    .with_stream(true);

    let mut stream = client.chat_stream(Provider::OpenAi, request).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.delta);
    }

    Ok(())
}
```

## 鉴权说明

- 统一 `ClientBuilder` 默认鉴权方式：
- OpenAI：`Authorization: Bearer <KEY>`
- Gemini：`x-goog-api-key: <KEY>`
- Claude：`Authorization: <KEY>`
- 各 Provider 专用客户端也遵循相同默认行为。

如果你的网关部署要求不同，可以通过 `ClientBuilder` 或各 Provider 构造器覆盖鉴权模式。

如果聊天模型与 ChatGPT 生图使用不同 key，可以在统一客户端上显式指定：

```rust
use yls_agi_rust_sdk::Client;

let client = Client::builder("your-yls-agi-key")
    .with_chatgpt_image_api_key("your-yls-codex-key")
    .build()?;
```

## 默认环境变量

- 网关聊天能力环境变量：`YLS_AGI_KEY`
- ChatGPT 生图环境变量：`YLS_CODEX_KEY`
- 统一客户端：`Client::from_env()?` 或 `Client::default()`
- `Client::from_env()` / `ClientBuilder::from_env()` 需要同时存在 `YLS_AGI_KEY` 和 `YLS_CODEX_KEY`
- Provider 客户端：`OpenAiClient::from_env()?`、`GeminiClient::from_env()?`、`ClaudeClient::from_env()?`
- ChatGPT 生图客户端：`ChatGptImageClient::from_env()?`
- 默认请求选项：`GenerationOptions::default()`
- 代理控制：`ClientBuilder::without_proxy()`、`ClientBuilder::with_system_proxy()`、`ClientBuilder::with_proxy("http://127.0.0.1:7890")`

`Client::default()` 在缺少 `YLS_AGI_KEY` 或 `YLS_CODEX_KEY` 时会 panic。需要可失败构造时，优先使用 `from_env()`。

## 模型枚举

### OpenAI

| 枚举 | 模型字符串 | 备注 |
| --- | --- | --- |
| `OpenAiModel::Gpt41` | `gpt-4.1` | 旗舰模型 |
| `OpenAiModel::Gpt5Mini` | `gpt-5-mini` | 快速 GPT-5 |
| `OpenAiModel::Gpt51` | `gpt-5.1` | 旗舰模型 |
| `OpenAiModel::Gpt51Chat` | `gpt-5.1-chat` | Chat 优化模型 |
| `OpenAiModel::O4MiniDeepResearch` | `o4-mini-deep-research` | 更实惠的深度研究模型 |
| `OpenAiModel::DeepseekV32Exp` | `deepseek-v3.2-exp` | deepseek 系列最新模型 |
| `OpenAiModel::Gpt52` | `gpt-5.2` | 无额外备注 |
| `OpenAiModel::Gpt52Chat` | `gpt-5.2-chat` | 快速 |
| `OpenAiModel::Gpt54` | `gpt-5.4` | 旗舰模型 |

### Claude

| 枚举 | 模型字符串 | 备注 |
| --- | --- | --- |
| `ClaudeModel::ClaudeHaiku4520251001` | `claude-haiku-4-5-20251001` | 快速经济模型 |
| `ClaudeModel::ClaudeSonnet4520250929` | `claude-sonnet-4-5-20250929` | 旗舰模型 |
| `ClaudeModel::ClaudeOpus4520251101` | `claude-opus-4-5-20251101` | 高级模型 |
| `ClaudeModel::ClaudeSonnet46` | `claude-sonnet-4-6` | 旗舰模型 |
| `ClaudeModel::ClaudeOpus46` | `claude-opus-4-6` | 高级模型 |

### Gemini

| 枚举 | 模型字符串 | 备注 |
| --- | --- | --- |
| `GeminiModel::Gemini3ProPreview` | `gemini-3-pro-preview` | 高级模型 |
| `GeminiModel::Gemini3FlashPreview` | `gemini-3-flash-preview` | Gemini3 快速 |
| `GeminiModel::Gemini25FlashImage` | `gemini-2.5-flash-image` | Nano Banana |
| `GeminiModel::Gemini3ProImagePreview` | `gemini-3-pro-image-preview` | Nano Banana Pro |
| `GeminiModel::Gemini31ProPreview` | `gemini-3.1-pro-preview` | 高级模型 |
