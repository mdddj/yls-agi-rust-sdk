use crate::{
    error::{Error, Result},
    provider::{AuthMode, ChatProvider, ChatStream, ProviderConfig, sse},
    types::{ChatMessage, ChatRequest, ChatResponse, FinishReason, Role, Usage},
};
use futures::StreamExt;
use openai_api_rust::{
    Message, Role as OpenAiRole,
    apis::chat::{ChatApi, ChatBody},
    openai::{Auth, OpenAI},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::spawn_blocking;
use url::Url;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    config: ProviderConfig,
}

impl OpenAiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url_and_auth(
            api_key,
            Url::parse("https://api.ylsagi.com/openai/v1/")?,
            AuthMode::AuthorizationBearer,
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
        let config = ProviderConfig {
            base_url,
            api_key: api_key.into(),
            auth_mode,
            http_client: reqwest::Client::new(),
        };
        Ok(Self { config })
    }

    fn to_openai_messages(messages: &[ChatMessage]) -> Vec<Message> {
        messages
            .iter()
            .filter(|message| message.role != Role::System)
            .map(|message| Message {
                role: match message.role {
                    Role::User => OpenAiRole::User,
                    Role::Assistant => OpenAiRole::Assistant,
                    Role::System => OpenAiRole::System,
                },
                content: message.content.clone(),
            })
            .collect()
    }

    fn system_prompt(messages: &[ChatMessage]) -> Option<String> {
        let prompts = messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(|message| message.content.clone())
            .collect::<Vec<_>>();

        if prompts.is_empty() {
            None
        } else {
            Some(prompts.join("\n"))
        }
    }

    pub async fn raw_chat_completion(&self, request: ChatRequest) -> Result<Value> {
        if request.stream {
            return Err(Error::UnsupportedConfig(
                "raw_chat_completion only supports non-stream requests".to_string(),
            ));
        }

        let base_url = self.config.base_url.to_string();
        let api_key = self.config.api_key.clone();
        let auth_mode = self.config.auth_mode.clone();
        let body = ChatBody {
            model: request.model,
            messages: Self::to_openai_messages(&request.messages),
            temperature: request.options.temperature,
            top_p: request.options.top_p,
            n: Some(1),
            stream: Some(false),
            stop: request.options.stop,
            max_tokens: request.options.max_tokens.map(|value| value as i32),
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: request
                .options
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user").cloned()),
        };

        let system_prompt = Self::system_prompt(&request.messages);
        let completion = spawn_blocking(move || {
            if auth_mode != AuthMode::AuthorizationBearer {
                return Err(Error::UnsupportedConfig(
                    "openai_api_rust only supports bearer authorization".to_string(),
                ));
            }

            let openai = OpenAI::new(Auth::new(&api_key), &base_url);
            let mut body = body;
            if let Some(prompt) = system_prompt {
                body.messages.insert(
                    0,
                    Message {
                        role: OpenAiRole::System,
                        content: prompt,
                    },
                );
            }
            openai
                .chat_completion_create(&body)
                .map_err(|err| Error::Provider(err.to_string()))
        })
        .await??;

        Ok(serde_json::to_value(completion)?)
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        <Self as ChatProvider>::chat(self, request).await
    }

    pub async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        <Self as ChatProvider>::chat_stream(self, request).await
    }

    async fn raw_stream_completion(&self, request: &ChatRequest) -> Result<reqwest::Response> {
        #[derive(Debug, Serialize)]
        struct RequestBody<'a> {
            model: &'a str,
            messages: Vec<WireMessage<'a>>,
            stream: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            temperature: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            top_p: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            max_tokens: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stop: Option<&'a Vec<String>>,
        }

        #[derive(Debug, Serialize)]
        struct WireMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        let body = RequestBody {
            model: &request.model,
            messages: request
                .messages
                .iter()
                .map(|message| WireMessage {
                    role: match message.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    },
                    content: &message.content,
                })
                .collect(),
            stream: true,
            temperature: request.options.temperature,
            top_p: request.options.top_p,
            max_tokens: request.options.max_tokens,
            stop: request.options.stop.as_ref(),
        };

        let url = self.config.base_url.join("chat/completions")?;
        let request = self.config.auth_mode.apply(
            self.config.http_client.post(url).json(&body),
            &self.config.api_key,
        );
        Ok(request.send().await?.error_for_status()?)
    }

    fn normalize(raw: Value) -> ChatResponse {
        #[derive(Debug, Deserialize)]
        struct Completion {
            model: Option<String>,
            choices: Vec<Choice>,
            usage: Option<WireUsage>,
        }

        #[derive(Debug, Deserialize)]
        struct Choice {
            finish_reason: Option<String>,
            message: Option<WireMessage>,
        }

        #[derive(Debug, Deserialize)]
        struct WireMessage {
            content: String,
        }

        #[derive(Debug, Deserialize)]
        struct WireUsage {
            prompt_tokens: Option<u32>,
            completion_tokens: Option<u32>,
            total_tokens: Option<u32>,
        }

        let completion = serde_json::from_value::<Completion>(raw.clone()).ok();
        let choice = completion
            .as_ref()
            .and_then(|completion| completion.choices.first());

        ChatResponse {
            model: completion
                .as_ref()
                .and_then(|completion| completion.model.clone()),
            message: ChatMessage::assistant(
                choice
                    .and_then(|choice| choice.message.as_ref())
                    .map(|message| message.content.clone())
                    .unwrap_or_default(),
            ),
            finish_reason: choice
                .and_then(|choice| choice.finish_reason.as_deref())
                .map(map_finish_reason),
            usage: completion.and_then(|completion| {
                completion.usage.map(|usage| Usage {
                    input_tokens: usage.prompt_tokens,
                    output_tokens: usage.completion_tokens,
                    total_tokens: usage.total_tokens,
                })
            }),
            raw: Some(raw),
        }
    }
}

impl Default for OpenAiClient {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!("failed to build default OpenAiClient from {DEFAULT_API_KEY_ENV}: {err}")
        })
    }
}

pub(crate) fn map_finish_reason(value: &str) -> FinishReason {
    match value {
        "stop" => FinishReason::Stop,
        "length" => FinishReason::Length,
        "content_filter" => FinishReason::ContentFilter,
        "tool_calls" => FinishReason::ToolCall,
        other => FinishReason::Other(other.to_string()),
    }
}

#[async_trait::async_trait]
impl ChatProvider for OpenAiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let raw = self.raw_chat_completion(request).await?;
        Ok(Self::normalize(raw))
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        let response = self.raw_stream_completion(&request).await?;
        Ok(Box::pin(
            sse::parse_sse_stream(response, sse::openai_chunk_mapper).boxed(),
        ))
    }
}
