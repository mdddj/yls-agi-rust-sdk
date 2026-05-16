---
name: yls-agi-rust-sdk
description: Use when the user wants to write, review, or modify Rust code that uses the yls-agi-rust-sdk crate for the YLS AGI gateway. Covers unified Client usage, provider-specific OpenAI/Gemini/Claude clients, chat, streaming, multimodal image input, Gemini image generation and reference-image editing, model enums, auth defaults, and local verification in this repository.
---

# YLS AGI Rust SDK

Use this skill when the task is about calling the 伊莉思 gateway from Rust with this crate, or when changing the crate itself.

## First Read

Start with these local files:

- `README.md` for supported usage patterns and examples
- `src/lib.rs` for the public export surface
- `src/types.rs` for request/response types and multimodal helpers
- `src/models.rs` for model enums
- `src/provider/openai.rs`, `src/provider/gemini.rs`, `src/provider/claude.rs` for provider-specific wire formats
- `examples/` for runnable live usage
- `tests/sdk.rs` for expected request shapes

## Default Setup

- Environment variable: `YLS_AGI_KEY`
- Fallible constructors:
  - `Client::from_env()?`
  - `ClientBuilder::from_env()?`
  - `OpenAiClient::from_env()?`
  - `GeminiClient::from_env()?`
  - `ClaudeClient::from_env()?`
- `Default` exists for `Client`, provider clients, and `GenerationOptions`, but `Default` on clients panics if `YLS_AGI_KEY` is missing.

## Main API Surface

- Unified gateway client:
  - `Client`
  - `ClientBuilder`
  - `Provider::{OpenAi, Gemini, Claude}`
- Unified chat types:
  - `ChatRequest`
  - `ChatMessage`
  - `GenerationOptions`
  - `ChatResponse`
  - `ChatChunk`
- Multimodal helpers:
  - `ChatMessage::with_image_bytes(...)`
  - `ChatMessage::with_image_base64(...)`
  - `ChatMessage::with_image_url(...)`
  - `ImageMimeType`
  - `ImageMime`
- Gemini image generation:
  - `GeminiImageRequest`
  - `GeminiImageResponse`
  - `GeneratedImage`

Read text replies with `response.message.text_content()`.

## Provider Defaults

- OpenAI:
  - Path: `/openai/v1/chat/completions`
  - Default auth: `Authorization: Bearer <KEY>`
- Gemini:
  - Path: `/gemini/v1beta/models/{model}:{action}`
  - Default auth: `x-goog-api-key: <KEY>`
- Claude:
  - Path: `/claude/v1/messages`
  - Default auth: raw `Authorization: <KEY>`

If the gateway deployment differs, override auth/base URL through `ClientBuilder` or provider constructors.

## Usage Patterns

### Basic chat

Use `ChatRequest::new(model, messages)` and optionally `.with_options(...)`.

### Streaming

Use `.with_stream(true)` and consume `client.chat_stream(...).await?` as a `futures::Stream`.

### Vision / multimodal chat

`ChatMessage.content` is multimodal. Prefer helper methods instead of constructing `MessagePart` manually.

- Common path: `.with_image_bytes(ImageMimeType::Png, &bytes)`
- Base64 path: `.with_image_base64(ImageMimeType::Png, data_base64)`
- URL path: `.with_image_url(url)`

Provider caveat:

- Claude only supports base64 image input in this SDK. `image_url` is rejected.

### Gemini image generation and editing

Use `GeminiClient::generate_image(...)`.

- Pure text-to-image:
  - `GeminiImageRequest::new(GeminiModel::Gemini25FlashImage, "...")`
- Reference-image editing:
  - `.with_reference_image_bytes(ImageMimeType::Png, &image_bytes)`
  - `.with_reference_image_base64(ImageMimeType::Png, image_base64)`

Reference images are serialized as Gemini `inlineData`.

## Model Selection

Use the enums in `src/models.rs` instead of raw strings when possible:

- `OpenAiModel`
- `GeminiModel`
- `ClaudeModel`

Image-generation Gemini models currently exposed by the SDK:

- `GeminiModel::Gemini25FlashImage`
- `GeminiModel::Gemini3ProImagePreview`

## Examples To Reuse

Prefer adapting an existing example before writing new sample code:

- `examples/basic_chat.rs`
- `examples/openai_vision.rs`
- `examples/gemini_vision.rs`
- `examples/claude_vision.rs`
- `examples/gemini_image.rs`
- `examples/gemini_image_edit.rs`

## Verification

Run these after SDK changes:

- `cargo fmt --all`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

For live network checks:

- `cargo run --example basic_chat`
- `cargo run --example openai_vision`
- `cargo run --example gemini_vision`
- `cargo run --example claude_vision`
- `cargo run --example gemini_image`
- `cargo run --example gemini_image_edit`
- `cargo test --test live_multimodal -- --ignored --nocapture`

## Change Discipline

- If you change public types or constructors, update `README.md` and relevant `examples/`.
- If you change provider wire formats, update `tests/sdk.rs`.
- If you add models, update both `src/models.rs` and the model table in `README.md`.
