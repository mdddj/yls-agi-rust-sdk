mod claude;
mod gemini;
mod openai;
mod responses;
mod sse;

pub use claude::ClaudeClient;
pub use gemini::GeminiClient;
pub use openai::OpenAiClient;
#[allow(deprecated)]
pub use responses::{ChatGptImageClient, ResponsesClient};

use crate::{
    error::Result,
    types::{ChatChunk, ChatRequest, ChatResponse},
};
use futures::stream::BoxStream;
use reqwest::Proxy;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Gemini,
    Claude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    AuthorizationKey,
    AuthorizationBearer,
    XGoogApiKey,
}

impl AuthMode {
    pub fn apply(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        match self {
            AuthMode::AuthorizationKey => builder.header("Authorization", api_key),
            AuthMode::AuthorizationBearer => {
                builder.header("Authorization", format!("Bearer {api_key}"))
            }
            AuthMode::XGoogApiKey => builder.header("x-goog-api-key", api_key),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub base_url: Url,
    pub api_key: String,
    pub auth_mode: AuthMode,
    pub http_client: reqwest::Client,
}

#[derive(Debug, Clone, Default)]
pub struct HttpClientConfig {
    pub proxy: Option<ProxyConfig>,
}

impl HttpClientConfig {
    pub fn build_client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder();

        if let Some(proxy) = &self.proxy {
            match proxy {
                ProxyConfig::UseSystem => {}
                ProxyConfig::Disable => {
                    builder = builder.no_proxy();
                }
                ProxyConfig::Custom(proxy_url) => {
                    builder = builder.proxy(Proxy::all(proxy_url)?);
                }
            }
        }

        Ok(builder.build()?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyConfig {
    UseSystem,
    Disable,
    Custom(String),
}

pub type ChatStream = BoxStream<'static, Result<ChatChunk>>;

#[async_trait::async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream>;
}
