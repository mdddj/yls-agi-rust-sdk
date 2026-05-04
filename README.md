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
yls-agi-rust-sdk = "0.1.0"
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

- OpenAI: `OpenAiModel::Gpt41`, `OpenAiModel::Gpt5Mini`, `OpenAiModel::Gpt51`, `OpenAiModel::Gpt51Chat`, `OpenAiModel::O4MiniDeepResearch`, `OpenAiModel::DeepseekV32Exp`, `OpenAiModel::Gpt52`, `OpenAiModel::Gpt52Chat`, `OpenAiModel::Gpt54`
- Claude: `ClaudeModel::ClaudeHaiku4520251001`, `ClaudeModel::ClaudeSonnet4520250929`, `ClaudeModel::ClaudeOpus4520251101`, `ClaudeModel::ClaudeSonnet46`, `ClaudeModel::ClaudeOpus46`
- Gemini: `GeminiModel::Gemini3ProPreview`, `GeminiModel::Gemini3FlashPreview`, `GeminiModel::Gemini25FlashImage`, `GeminiModel::Gemini3ProImagePreview`, `GeminiModel::Gemini31ProPreview`
