pub mod client;
pub mod error;
pub mod models;
pub mod provider;
pub mod types;

pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use models::{ClaudeModel, GeminiModel, OpenAiModel};
pub use provider::{
    AuthMode, ClaudeClient, GeminiClient, HttpClientConfig, OpenAiClient, Provider, ProxyConfig,
};
pub use types::{
    ChatChunk, ChatMessage, ChatRequest, ChatResponse, FinishReason, GeminiImageRequest,
    GeminiImageResponse, GeneratedImage, GenerationOptions, Role, Usage,
};
