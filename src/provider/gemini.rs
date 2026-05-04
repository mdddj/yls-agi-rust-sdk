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
    Gemini, GenerationConfig, GenerationResponse, Message as GeminiMessage, Model as GeminiApiModel,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{Duration, sleep};
use url::Url;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";
const IMAGE_RETRY_DELAYS_MS: [u64; 3] = [0, 250, 1000];

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
        .map_err(|err| Error::provider("gemini", err.to_string()))
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
            .map_err(|err| Error::provider("gemini", err.to_string()))
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

        let wire_request = Self::build_image_request(request);
        let response = self
            .execute_image_with_retry(&model_name, &wire_request)
            .await?;

        Self::normalize_image_response(response)
    }

    async fn execute_image_with_retry(
        &self,
        model: &str,
        body: &GeminiImageWireRequest,
    ) -> Result<Value> {
        let mut last_error = None;

        for delay_ms in IMAGE_RETRY_DELAYS_MS {
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }

            match self.send_image_request(model, body).await {
                Ok(response) => return Ok(response),
                Err(ImageRequestFailure::Retryable(err)) => last_error = Some(err),
                Err(ImageRequestFailure::Fatal(err)) => return Err(err),
            }
        }

        Err(last_error
            .unwrap_or_else(|| Error::provider("gemini", "image generation failed after retries")))
    }

    fn build_image_request(request: GeminiImageRequest) -> GeminiImageWireRequest {
        let generation_config = GeminiImageWireGenerationConfig {
            temperature: request.options.temperature,
            top_p: request.options.top_p,
            max_output_tokens: request.options.max_tokens,
            stop_sequences: request.options.stop,
            response_modalities: vec!["TEXT".to_string(), "IMAGE".to_string()],
        };

        GeminiImageWireRequest {
            contents: vec![GeminiImageWireContent {
                parts: vec![GeminiImageWirePart {
                    text: request.prompt,
                }],
            }],
            system_instruction: request
                .system_prompt
                .map(|system_prompt| GeminiImageWireContent {
                    parts: vec![GeminiImageWirePart {
                        text: system_prompt,
                    }],
                }),
            generation_config: Some(generation_config),
        }
    }

    async fn send_image_request(
        &self,
        model: &str,
        body: &GeminiImageWireRequest,
    ) -> std::result::Result<Value, ImageRequestFailure> {
        let url = self
            .config
            .base_url
            .join(&format!("models/{model}:generateContent"))
            .map_err(|err| {
                ImageRequestFailure::Fatal(Error::provider("gemini", err.to_string()))
            })?;

        let request = self.config.auth_mode.apply(
            self.config.http_client.post(url).json(body),
            &self.config.api_key,
        );

        let response = request.send().await.map_err(|err| {
            ImageRequestFailure::Retryable(Error::provider(
                "gemini",
                format!("image request transport error: {err}"),
            ))
        })?;
        let status = response.status();
        let response_body = response.text().await.map_err(|err| {
            let error = Error::provider(
                "gemini",
                format!("failed to read image response body (status {status}): {err}"),
            );

            if is_retryable_status(status) {
                ImageRequestFailure::Retryable(error)
            } else {
                ImageRequestFailure::Fatal(error)
            }
        })?;

        if !status.is_success() {
            let error = Error::provider(
                "gemini",
                format!(
                    "image request failed with status {status}: {}",
                    truncate_error_body(&response_body)
                ),
            );

            return Err(if is_retryable_status(status) {
                ImageRequestFailure::Retryable(error)
            } else {
                ImageRequestFailure::Fatal(error)
            });
        }

        serde_json::from_str(&response_body).map_err(|err| {
            ImageRequestFailure::Fatal(Error::provider(
                "gemini",
                format!(
                    "failed to decode image generation response: {err}; body: {}",
                    truncate_error_body(&response_body)
                ),
            ))
        })
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

    fn normalize_image_response(raw: Value) -> Result<GeminiImageResponse> {
        let response: GeminiImageWireResponse =
            serde_json::from_value(raw.clone()).map_err(|err| {
                Error::provider(
                    "gemini",
                    format!("failed to normalize image generation response: {err}"),
                )
            })?;
        let model = response.model_version;
        let mut text = Vec::new();
        let mut images = Vec::new();

        for candidate in response.candidates {
            if let Some(content) = candidate.content
                && let Some(parts) = content.parts
            {
                for part in parts {
                    match part {
                        GeminiImageWireResponsePart::Text { text: value } => text.push(value),
                        GeminiImageWireResponsePart::InlineData { inline_data } => {
                            images.push(GeneratedImage {
                                mime_type: inline_data.mime_type.clone(),
                                data_base64: inline_data.data.clone(),
                                bytes: BASE64.decode(&inline_data.data)?,
                            })
                        }
                        GeminiImageWireResponsePart::Other => {}
                    }
                }
            }
        }

        Ok(GeminiImageResponse {
            model,
            text,
            images,
            raw: Some(raw),
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireRequest {
    contents: Vec<GeminiImageWireContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiImageWireContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiImageWireGenerationConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireContent {
    parts: Vec<GeminiImageWirePart>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWirePart {
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    response_modalities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireResponse {
    #[serde(default)]
    candidates: Vec<GeminiImageWireCandidate>,
    model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireCandidate {
    content: Option<GeminiImageWireResponseContent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireResponseContent {
    parts: Option<Vec<GeminiImageWireResponsePart>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GeminiImageWireResponsePart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiImageWireInlineData,
    },
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageWireInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug)]
enum ImageRequestFailure {
    Retryable(Error),
    Fatal(Error),
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || matches!(status.as_u16(), 408 | 409 | 425 | 429)
}

fn truncate_error_body(body: &str) -> String {
    const MAX_LEN: usize = 800;

    if body.len() <= MAX_LEN {
        body.to_string()
    } else {
        format!("{}...(truncated)", &body[..MAX_LEN])
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
            .map_err(|err| Error::provider("gemini", err.to_string()))?
            .map_err(|err| Error::provider("gemini", err.to_string()))
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
