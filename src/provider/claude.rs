use crate::{
    error::{Error, Result},
    provider::{AuthMode, ChatProvider, ChatStream, HttpClientConfig, ProviderConfig, sse},
    types::{ChatMessage, ChatRequest, ChatResponse, FinishReason, MessagePart, Role},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";

#[derive(Debug, Clone)]
pub struct ClaudeClient {
    config: ProviderConfig,
}

impl ClaudeClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url_and_auth(
            api_key,
            Url::parse("https://api.ylsagi.com/claude/v1/")?,
            AuthMode::AuthorizationKey,
        )
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(DEFAULT_API_KEY_ENV)
            .map_err(|_| Error::MissingEnvVar(DEFAULT_API_KEY_ENV))?;
        Self::new(api_key)
    }

    pub fn with_base_url_and_auth(
        api_key: impl Into<String>,
        base_url: Url,
        auth_mode: AuthMode,
    ) -> Result<Self> {
        Self::with_config(api_key, base_url, auth_mode, HttpClientConfig::default())
    }

    pub fn with_config(
        api_key: impl Into<String>,
        base_url: Url,
        auth_mode: AuthMode,
        http_config: HttpClientConfig,
    ) -> Result<Self> {
        Ok(Self {
            config: ProviderConfig {
                base_url,
                api_key: api_key.into(),
                auth_mode,
                http_client: http_config.build_client()?,
            },
        })
    }

    pub async fn raw_messages(&self, request: ChatRequest) -> Result<Value> {
        let response = self.send_request(&request, false).await?;
        Ok(response.json().await?)
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        <Self as ChatProvider>::chat(self, request).await
    }

    pub async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        <Self as ChatProvider>::chat_stream(self, request).await
    }

    async fn send_request(&self, request: &ChatRequest, stream: bool) -> Result<reqwest::Response> {
        #[derive(Debug, Serialize)]
        struct Body<'a> {
            model: &'a str,
            stream: bool,
            max_tokens: u32,
            messages: Vec<Message>,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            temperature: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            top_p: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stop_sequences: Option<&'a Vec<String>>,
        }

        #[derive(Debug, Serialize)]
        struct Message {
            role: &'static str,
            content: Vec<ContentBlock>,
        }

        #[derive(Debug, Serialize)]
        #[serde(tag = "type")]
        enum ContentBlock {
            #[serde(rename = "text")]
            Text { text: String },
            #[serde(rename = "image")]
            Image { source: ImageSource },
        }

        #[derive(Debug, Serialize)]
        struct ImageSource {
            #[serde(rename = "type")]
            source_type: &'static str,
            media_type: String,
            data: String,
        }

        if request
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .any(|part| matches!(part, MessagePart::ImageUrl { .. }))
        {
            return Err(Error::UnsupportedConfig(
                "claude image_url input is not supported; use base64 image data".to_string(),
            ));
        }

        let system = request
            .messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(ChatMessage::text_content)
            .collect::<Vec<_>>();
        let system = (!system.is_empty()).then(|| system.join("\n"));

        let body = Body {
            model: &request.model,
            stream,
            max_tokens: request.options.max_tokens.unwrap_or(8192),
            messages: request
                .messages
                .iter()
                .filter(|message| message.role != Role::System)
                .map(|message| Message {
                    role: match message.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => "system",
                    },
                    content: message
                        .content
                        .iter()
                        .map(|part| match part {
                            MessagePart::Text { text } => ContentBlock::Text { text: text.clone() },
                            MessagePart::ImageBase64 {
                                mime_type,
                                data_base64,
                            } => ContentBlock::Image {
                                source: ImageSource {
                                    source_type: "base64",
                                    media_type: mime_type.clone(),
                                    data: data_base64.clone(),
                                },
                            },
                            MessagePart::ImageUrl { .. } => unreachable!(),
                        })
                        .collect(),
                })
                .collect(),
            system,
            temperature: request.options.temperature,
            top_p: request.options.top_p,
            stop_sequences: request.options.stop.as_ref(),
        };

        let url = self.config.base_url.join("messages")?;
        let request = self.config.auth_mode.apply(
            self.config.http_client.post(url).json(&body),
            &self.config.api_key,
        );
        Ok(request.send().await?.error_for_status()?)
    }

    fn normalize(raw: Value) -> ChatResponse {
        #[derive(Debug, Deserialize)]
        struct Response {
            model: Option<String>,
            content: Option<Vec<Content>>,
            stop_reason: Option<String>,
            usage: Option<WireUsage>,
        }

        #[derive(Debug, Deserialize)]
        struct Content {
            text: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        struct WireUsage {
            input_tokens: Option<u32>,
            output_tokens: Option<u32>,
        }

        let response = serde_json::from_value::<Response>(raw.clone()).ok();
        let content = response
            .as_ref()
            .and_then(|value| value.content.as_ref())
            .and_then(|content| content.first())
            .and_then(|content| content.text.clone())
            .unwrap_or_default();

        ChatResponse {
            model: response.as_ref().and_then(|value| value.model.clone()),
            message: ChatMessage::assistant(content),
            finish_reason: response
                .as_ref()
                .and_then(|value| value.stop_reason.as_deref())
                .map(|reason| match reason {
                    "end_turn" | "stop_sequence" => FinishReason::Stop,
                    "max_tokens" => FinishReason::Length,
                    other => FinishReason::Other(other.to_string()),
                }),
            usage: response.and_then(|value| {
                value.usage.map(|usage| crate::types::Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    total_tokens: match (usage.input_tokens, usage.output_tokens) {
                        (Some(input), Some(output)) => Some(input + output),
                        _ => None,
                    },
                })
            }),
            raw: Some(raw),
        }
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!("failed to build default ClaudeClient from {DEFAULT_API_KEY_ENV}: {err}")
        })
    }
}

#[async_trait::async_trait]
impl ChatProvider for ClaudeClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        Ok(Self::normalize(self.raw_messages(request).await?))
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        let response = self.send_request(&request, true).await?;
        Ok(Box::pin(
            sse::parse_sse_stream(response, sse::claude_chunk_mapper).boxed(),
        ))
    }
}
