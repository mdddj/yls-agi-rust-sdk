# yls-agi-rust-sdk

Rust SDK for the 伊莉思 API gateway with a unified async client and provider-specific clients.

## Features

- Unified `Client` for OpenAI, Gemini, and Claude chat requests
- Provider-specific clients when raw access is needed
- Async Tokio API
- Streaming support for OpenAI, Gemini, and Claude
- Pluggable auth modes to handle gateway compatibility differences

## Install

```toml
[dependencies]
yls-agi-rust-sdk = "0.1.1"
```

## Quick Start

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, Client, Provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let request = ChatRequest::new(
        "gpt-4.1",
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("Say hello in Chinese."),
        ],
    );

    let response = client.chat(Provider::OpenAi, request).await?;
    println!("{}", response.message.content);
    Ok(())
}
```

## OpenAI Example

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
    println!("{}", response.message.content);
    Ok(())
}
```

## Gemini Example

```rust
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, GeminiClient, GeminiModel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiClient::from_env()?;
    let request = ChatRequest::new(
        GeminiModel::Gemini3FlashPreview,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("Explain ownership in one sentence."),
        ],
    );

    let response = client.chat(request).await?;
    println!("{}", response.message.content);
    Ok(())
}
```

## Gemini Image Example

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
                "Create a photorealistic orange cat wearing sunglasses.",
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

## Claude Example

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
    println!("{}", response.message.content);
    Ok(())
}
```

## Streaming Example

```rust
use futures::StreamExt;
use yls_agi_rust_sdk::{ChatMessage, ChatRequest, Client, Provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let request = ChatRequest::new(
        "gpt-4.1",
        vec![ChatMessage::user("Write a short greeting letter.")],
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

## Auth Notes

- OpenAI defaults to `Authorization: Bearer <KEY>` because `openai_api_rust` hardcodes bearer auth.
- Gemini defaults to `x-goog-api-key: <KEY>` because `gemini-rust` hardcodes Google-style auth.
- Claude defaults to raw `Authorization: <KEY>` which matches the 伊莉思 docs.

Use `ClientBuilder` or provider constructors to override auth mode if your gateway deployment differs.

## Default Env

- Default environment variable: `YLS_AGI_KEY`
- Unified client: `Client::from_env()?` or `Client::default()`
- Provider clients: `OpenAiClient::from_env()?`, `GeminiClient::from_env()?`, `ClaudeClient::from_env()?`
- Request options: `GenerationOptions::default()`

`Default` will panic if `YLS_AGI_KEY` is missing. Prefer `from_env()` when you want a fallible constructor.

## Model Enums

### OpenAI

| Enum | Model String | 文档备注 |
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

| Enum | Model String | 文档备注 |
| --- | --- | --- |
| `ClaudeModel::ClaudeHaiku4520251001` | `claude-haiku-4-5-20251001` | 快速经济模型 |
| `ClaudeModel::ClaudeSonnet4520250929` | `claude-sonnet-4-5-20250929` | 旗舰模型 |
| `ClaudeModel::ClaudeOpus4520251101` | `claude-opus-4-5-20251101` | 高级模型 |
| `ClaudeModel::ClaudeSonnet46` | `claude-sonnet-4-6` | 旗舰模型 |
| `ClaudeModel::ClaudeOpus46` | `claude-opus-4-6` | 高级模型 |

### Gemini

| Enum | Model String | 文档备注 |
| --- | --- | --- |
| `GeminiModel::Gemini3ProPreview` | `gemini-3-pro-preview` | 高级模型 |
| `GeminiModel::Gemini3FlashPreview` | `gemini-3-flash-preview` | Gemini3 快速 |
| `GeminiModel::Gemini25FlashImage` | `gemini-2.5-flash-image` | Nano Banana |
| `GeminiModel::Gemini3ProImagePreview` | `gemini-3-pro-image-preview` | Nano Banana Pro |
| `GeminiModel::Gemini31ProPreview` | `gemini-3.1-pro-preview` | 高级模型 |
