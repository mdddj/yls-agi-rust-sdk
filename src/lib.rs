pub mod client;
pub mod error;
pub mod models;
pub mod provider;
pub mod types;

pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use models::{ClaudeModel, GeminiModel, OpenAiModel};
#[allow(deprecated)]
pub use provider::{
    AuthMode, ChatGptImageClient, ClaudeClient, GeminiClient, HttpClientConfig, OpenAiClient,
    Provider, ProxyConfig, ResponsesClient,
};
#[allow(deprecated)]
pub use types::{
    ChatChunk, ChatGptImageRequest, ChatGptImageResponse, ChatGptReferenceImage, ChatMessage,
    ChatRequest, ChatResponse, FinishReason, GeminiImageRequest, GeminiImageResponse,
    GeminiReferenceImage, GeneratedImage, GenerationOptions, ImageMime, ImageMimeType, MessagePart,
    ResponsesImageRequest, ResponsesImageResponse, ResponsesReferenceImage, Role, SavedImageInfo,
    Usage, format_duration_ms,
};
