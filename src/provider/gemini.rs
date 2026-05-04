use crate::{
    error::{Error, Result},
    models::GeminiModel,
    provider::{AuthMode, ChatProvider, ChatStream, ProviderConfig},
    types::{
        ChatChunk, ChatMessage, ChatRequest, ChatResponse, FinishReason, GeminiImageRequest,
        GeminiImageResponse, GeneratedImage, Role, Usage,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use futures::{StreamExt, TryStreamExt};
use gemini_rust::{
    Gemini, GenerationConfig, GenerationResponse, Message as GeminiMessage,
    Model as GeminiApiModel, Part,
};
use serde_json::Value;
use url::Url;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";

#[derive(Debug, Clone)]
pub struct GeminiClient {
    config: ProviderConfig,
}

impl GeminiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url_and_auth(
            api_key,
            Url::parse("https://api.ylsagi.com/gemini/v1beta/")?,
            AuthMode::XGoogApiKey,
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
        Ok(Self {
            config: ProviderConfig {
                base_url,
                api_key: api_key.into(),
                auth_mode,
                http_client: reqwest::Client::new(),
            },
        })
    }

    pub async fn raw_generate_content(&self, request: ChatRequest) -> Result<Value> {
        let response = self.execute(request).await?;
        Ok(serde_json::to_value(response)?)
    }

    fn build_client(&self, model: &str) -> Result<Gemini> {
        Gemini::with_model_and_base_url(
            self.config.api_key.clone(),
            GeminiApiModel::Custom(format!("models/{model}")),
            self.config.base_url.clone(),
        )
        .map_err(|err| crate::error::Error::Provider(err.to_string()))
    }

    fn build_messages(messages: &[ChatMessage]) -> Vec<GeminiMessage> {
        messages
            .iter()
            .filter(|message| message.role != Role::System)
            .map(|message| match message.role {
                Role::User => GeminiMessage::user(message.content.clone()),
                Role::Assistant => GeminiMessage::model(message.content.clone()),
                Role::System => GeminiMessage::user(message.content.clone()),
            })
            .collect()
    }

    async fn execute(&self, request: ChatRequest) -> Result<GenerationResponse> {
        let client = self.build_client(&request.model)?;
        let system = request
            .messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        let mut builder = client
            .generate_content()
            .with_messages(Self::build_messages(&request.messages))
            .with_generation_config(GenerationConfig {
                temperature: request.options.temperature,
                top_p: request.options.top_p,
                max_output_tokens: request.options.max_tokens.map(|value| value as i32),
                stop_sequences: request.options.stop,
                ..Default::default()
            });

        if !system.is_empty() {
            builder = builder.with_system_prompt(system);
        }

        builder
            .execute()
            .await
            .map_err(|err| crate::error::Error::Provider(err.to_string()))
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        <Self as ChatProvider>::chat(self, request).await
    }

    pub async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        <Self as ChatProvider>::chat_stream(self, request).await
    }

    pub async fn generate_image(&self, request: GeminiImageRequest) -> Result<GeminiImageResponse> {
        let model_name = request.model.clone();
        let supports_image = [
            GeminiModel::Gemini25FlashImage.as_str(),
            GeminiModel::Gemini3ProImagePreview.as_str(),
        ]
        .contains(&model_name.as_str());

        if !supports_image {
            return Err(crate::error::Error::UnsupportedConfig(format!(
                "model `{model_name}` does not support image generation"
            )));
        }

        let client = self.build_client(&request.model)?;
        let mut builder = client
            .generate_content()
            .with_user_message(request.prompt)
            .with_generation_config(GenerationConfig {
                temperature: request.options.temperature,
                top_p: request.options.top_p,
                max_output_tokens: request.options.max_tokens.map(|value| value as i32),
                stop_sequences: request.options.stop,
                ..Default::default()
            });

        if let Some(system_prompt) = request.system_prompt {
            builder = builder.with_system_prompt(system_prompt);
        }

        let response = builder
            .execute()
            .await
            .map_err(|err| crate::error::Error::Provider(err.to_string()))?;

        Self::normalize_image_response(response)
    }

    fn normalize(response: GenerationResponse) -> ChatResponse {
        let usage = response.usage_metadata.as_ref().map(|usage| Usage {
            input_tokens: usage.prompt_token_count.map(|value| value as u32),
            output_tokens: usage.candidates_token_count.map(|value| value as u32),
            total_tokens: usage.total_token_count.map(|value| value as u32),
        });
        let model = response.model_version.clone();
        let text = response.text();
        let raw = serde_json::to_value(&response).ok();

        ChatResponse {
            model,
            message: ChatMessage::assistant(text),
            finish_reason: response
                .candidates
                .first()
                .and_then(|candidate| candidate.finish_reason.as_ref())
                .map(|reason| FinishReason::Other(format!("{reason:?}"))),
            usage,
            raw,
        }
    }

    fn normalize_image_response(response: GenerationResponse) -> Result<GeminiImageResponse> {
        let model = response.model_version.clone();
        let raw = serde_json::to_value(&response).ok();
        let mut text = Vec::new();
        let mut images = Vec::new();

        for candidate in &response.candidates {
            if let Some(parts) = &candidate.content.parts {
                for part in parts {
                    match part {
                        Part::Text { text: value, .. } => text.push(value.clone()),
                        Part::InlineData { inline_data, .. } => images.push(GeneratedImage {
                            mime_type: inline_data.mime_type.clone(),
                            data_base64: inline_data.data.clone(),
                            bytes: BASE64.decode(&inline_data.data)?,
                        }),
                        _ => {}
                    }
                }
            }
        }

        Ok(GeminiImageResponse {
            model,
            text,
            images,
            raw,
        })
    }
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!("failed to build default GeminiClient from {DEFAULT_API_KEY_ENV}: {err}")
        })
    }
}

#[async_trait::async_trait]
impl ChatProvider for GeminiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        Ok(Self::normalize(self.execute(request).await?))
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        let client = self.build_client(&request.model)?;
        let system = request
            .messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        let mut builder = client
            .generate_content()
            .with_messages(Self::build_messages(&request.messages))
            .with_generation_config(GenerationConfig {
                temperature: request.options.temperature,
                top_p: request.options.top_p,
                max_output_tokens: request.options.max_tokens.map(|value| value as i32),
                stop_sequences: request.options.stop,
                ..Default::default()
            });

        if !system.is_empty() {
            builder = builder.with_system_prompt(system);
        }

        let stream = builder
            .execute_stream()
            .await
            .map_err(|err| crate::error::Error::Provider(err.to_string()))?
            .map_err(|err| crate::error::Error::Provider(err.to_string()))
            .map(|chunk| {
                chunk.map(|response| ChatChunk {
                    delta: response.text(),
                    done: false,
                    finish_reason: None,
                    raw: serde_json::to_value(response).ok(),
                })
            });

        Ok(Box::pin(stream))
    }
}
