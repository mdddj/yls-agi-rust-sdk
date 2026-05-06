use crate::{
    error::{Error, Result},
    models::GeminiModel,
    provider::{AuthMode, ChatProvider, ChatStream, HttpClientConfig, ProviderConfig, sse},
    types::{
        ChatChunk, ChatMessage, ChatRequest, ChatResponse, FinishReason, GeminiImageRequest,
        GeminiImageResponse, GeneratedImage, GenerationOptions, MessagePart, Role, Usage,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use futures::StreamExt;
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

    pub async fn raw_generate_content(&self, request: ChatRequest) -> Result<Value> {
        let body = Self::build_chat_request(&request);
        self.send_json_request(&request.model, "generateContent", &body)
            .await
            .map_err(HttpRequestFailure::into_error)
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
            return Err(Error::UnsupportedConfig(format!(
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
        body: &GeminiGenerateContentRequest,
    ) -> Result<Value> {
        let mut last_error = None;

        for delay_ms in IMAGE_RETRY_DELAYS_MS {
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }

            match self.send_json_request(model, "generateContent", body).await {
                Ok(response) => return Ok(response),
                Err(HttpRequestFailure::Retryable(err)) => last_error = Some(err),
                Err(HttpRequestFailure::Fatal(err)) => return Err(err),
            }
        }

        Err(last_error
            .unwrap_or_else(|| Error::provider("gemini", "image generation failed after retries")))
    }

    fn build_chat_request(request: &ChatRequest) -> GeminiGenerateContentRequest {
        GeminiGenerateContentRequest {
            contents: Self::build_contents(&request.messages),
            system_instruction: Self::build_system_instruction(&request.messages),
            generation_config: Self::build_generation_config(&request.options, None),
        }
    }

    fn build_image_request(request: GeminiImageRequest) -> GeminiGenerateContentRequest {
        let mut parts = Vec::with_capacity(1 + request.reference_images.len());
        parts.push(GeminiRequestPart::Text {
            text: request.prompt,
        });
        parts.extend(request.reference_images.into_iter().map(|image| {
            GeminiRequestPart::InlineData {
                inline_data: GeminiInlineData {
                    mime_type: image.mime_type,
                    data: image.data_base64,
                },
            }
        }));

        GeminiGenerateContentRequest {
            contents: vec![GeminiRequestContent { role: None, parts }],
            system_instruction: request
                .system_prompt
                .map(Self::system_instruction_from_text),
            generation_config: Self::build_generation_config(
                &request.options,
                Some(vec!["TEXT".to_string(), "IMAGE".to_string()]),
            ),
        }
    }

    fn build_contents(messages: &[ChatMessage]) -> Vec<GeminiRequestContent> {
        messages
            .iter()
            .filter(|message| message.role != Role::System)
            .map(|message| GeminiRequestContent {
                role: Some(
                    match message.role {
                        Role::User => "user",
                        Role::Assistant => "model",
                        Role::System => "user",
                    }
                    .to_string(),
                ),
                parts: message.content.iter().map(map_request_part).collect(),
            })
            .collect()
    }

    fn build_system_instruction(messages: &[ChatMessage]) -> Option<GeminiRequestContent> {
        let system = messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(ChatMessage::text_content)
            .collect::<Vec<_>>()
            .join("\n");

        (!system.is_empty()).then(|| Self::system_instruction_from_text(system))
    }

    fn system_instruction_from_text(text: impl Into<String>) -> GeminiRequestContent {
        GeminiRequestContent {
            role: None,
            parts: vec![GeminiRequestPart::Text { text: text.into() }],
        }
    }

    fn build_generation_config(
        options: &GenerationOptions,
        response_modalities: Option<Vec<String>>,
    ) -> Option<GeminiRequestGenerationConfig> {
        let config = GeminiRequestGenerationConfig {
            temperature: options.temperature,
            top_p: options.top_p,
            max_output_tokens: options.max_tokens,
            stop_sequences: options.stop.clone(),
            response_modalities,
        };

        if config.temperature.is_none()
            && config.top_p.is_none()
            && config.max_output_tokens.is_none()
            && config.stop_sequences.is_none()
            && config.response_modalities.is_none()
        {
            None
        } else {
            Some(config)
        }
    }

    async fn send_json_request<T: Serialize>(
        &self,
        model: &str,
        action: &str,
        body: &T,
    ) -> std::result::Result<Value, HttpRequestFailure> {
        let url = self
            .build_action_url(model, action)
            .map_err(HttpRequestFailure::Fatal)?;
        let request = self.config.auth_mode.apply(
            self.config.http_client.post(url).json(body),
            &self.config.api_key,
        );

        let response = request.send().await.map_err(|err| {
            HttpRequestFailure::Retryable(Error::provider(
                "gemini",
                format!("request transport error: {err}"),
            ))
        })?;
        let status = response.status();
        let response_body = response.text().await.map_err(|err| {
            let error = Error::provider(
                "gemini",
                format!("failed to read response body (status {status}): {err}"),
            );
            if is_retryable_status(status) {
                HttpRequestFailure::Retryable(error)
            } else {
                HttpRequestFailure::Fatal(error)
            }
        })?;

        if !status.is_success() {
            let error = Error::provider(
                "gemini",
                format!(
                    "request failed with status {status}: {}",
                    truncate_error_body(&response_body)
                ),
            );
            return Err(if is_retryable_status(status) {
                HttpRequestFailure::Retryable(error)
            } else {
                HttpRequestFailure::Fatal(error)
            });
        }

        serde_json::from_str(&response_body).map_err(|err| {
            HttpRequestFailure::Fatal(Error::provider(
                "gemini",
                format!(
                    "failed to decode response: {err}; body: {}",
                    truncate_error_body(&response_body)
                ),
            ))
        })
    }

    async fn send_stream_request<T: Serialize>(
        &self,
        model: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let mut url = self.build_action_url(model, "streamGenerateContent")?;
        url.query_pairs_mut().append_pair("alt", "sse");

        let request = self.config.auth_mode.apply(
            self.config.http_client.post(url).json(body),
            &self.config.api_key,
        );

        let response = request
            .send()
            .await
            .map_err(|err| Error::provider("gemini", format!("stream transport error: {err}")))?;
        let status = response.status();

        if status.is_success() {
            return Ok(response);
        }

        let response_body = response.text().await.map_err(|err| {
            Error::provider(
                "gemini",
                format!("failed to read stream error body (status {status}): {err}"),
            )
        })?;

        Err(Error::provider(
            "gemini",
            format!(
                "stream request failed with status {status}: {}",
                truncate_error_body(&response_body)
            ),
        ))
    }

    fn build_action_url(&self, model: &str, action: &str) -> Result<Url> {
        self.config
            .base_url
            .join(&format!("models/{model}:{action}"))
            .map_err(Error::from)
    }

    fn normalize_chat_response(raw: Value) -> Result<ChatResponse> {
        let response: GeminiGenerateContentResponse =
            serde_json::from_value(raw.clone()).map_err(|err| {
                Error::provider(
                    "gemini",
                    format!("failed to normalize chat response: {err}"),
                )
            })?;

        let first_candidate = response.candidates.first();
        let content = first_candidate
            .and_then(|candidate| candidate.content.as_ref())
            .map(extract_text)
            .unwrap_or_default();

        Ok(ChatResponse {
            model: response.model_version,
            message: ChatMessage::assistant(content),
            finish_reason: first_candidate
                .and_then(|candidate| candidate.finish_reason.as_deref())
                .map(map_finish_reason),
            usage: response.usage_metadata.map(|usage| Usage {
                input_tokens: usage.prompt_token_count,
                output_tokens: usage.candidates_token_count,
                total_tokens: usage.total_token_count,
            }),
            raw: Some(raw),
        })
    }

    fn normalize_image_response(raw: Value) -> Result<GeminiImageResponse> {
        let response: GeminiGenerateContentResponse =
            serde_json::from_value(raw.clone()).map_err(|err| {
                Error::provider(
                    "gemini",
                    format!("failed to normalize image generation response: {err}"),
                )
            })?;
        let mut text = Vec::new();
        let mut images = Vec::new();

        for candidate in response.candidates {
            if let Some(content) = candidate.content
                && let Some(parts) = content.parts
            {
                for part in parts {
                    match part {
                        GeminiResponsePart::Text { text: value } => text.push(value),
                        GeminiResponsePart::InlineData { inline_data } => {
                            images.push(GeneratedImage {
                                mime_type: inline_data.mime_type.clone(),
                                data_base64: inline_data.data.clone(),
                                bytes: BASE64.decode(&inline_data.data)?,
                            })
                        }
                        GeminiResponsePart::Other(_) => {}
                    }
                }
            }
        }

        Ok(GeminiImageResponse {
            model: response.model_version,
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

#[async_trait::async_trait]
impl ChatProvider for GeminiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let body = Self::build_chat_request(&request);
        let raw = self
            .send_json_request(&request.model, "generateContent", &body)
            .await
            .map_err(HttpRequestFailure::into_error)?;
        Self::normalize_chat_response(raw)
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        let body = Self::build_chat_request(&request);
        let response = self.send_stream_request(&request.model, &body).await?;

        Ok(Box::pin(
            sse::parse_sse_stream(response, gemini_chunk_mapper).boxed(),
        ))
    }
}

fn gemini_chunk_mapper(data: String) -> Result<Option<ChatChunk>> {
    let raw: Value = serde_json::from_str(&data)?;
    let response: GeminiGenerateContentResponse = serde_json::from_value(raw.clone())
        .map_err(|err| Error::provider("gemini", format!("failed to parse stream chunk: {err}")))?;
    let candidate = match response.candidates.first() {
        Some(candidate) => candidate,
        None => return Ok(None),
    };
    let delta = candidate
        .content
        .as_ref()
        .map(extract_text)
        .unwrap_or_default();
    let finish_reason = candidate.finish_reason.as_deref().map(map_finish_reason);

    if delta.is_empty() && finish_reason.is_none() {
        return Ok(None);
    }

    Ok(Some(ChatChunk {
        done: finish_reason.is_some(),
        delta,
        finish_reason,
        raw: Some(raw),
    }))
}

fn extract_text(content: &GeminiResponseContent) -> String {
    content
        .parts
        .as_ref()
        .into_iter()
        .flat_map(|parts| parts.iter())
        .filter_map(|part| match part {
            GeminiResponsePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn map_request_part(part: &MessagePart) -> GeminiRequestPart {
    match part {
        MessagePart::Text { text } => GeminiRequestPart::Text { text: text.clone() },
        MessagePart::ImageUrl { url } => GeminiRequestPart::FileData {
            file_data: GeminiFileData {
                mime_type: guess_mime_type_from_url(url),
                file_uri: url.clone(),
            },
        },
        MessagePart::ImageBase64 {
            mime_type,
            data_base64,
        } => GeminiRequestPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: mime_type.clone(),
                data: data_base64.clone(),
            },
        },
    }
}

fn map_finish_reason(value: &str) -> FinishReason {
    match value {
        "STOP" => FinishReason::Stop,
        "MAX_TOKENS" => FinishReason::Length,
        "SAFETY" | "RECITATION" | "LANGUAGE" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII"
        | "IMAGE_SAFETY" => FinishReason::ContentFilter,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "TOO_MANY_TOOL_CALLS" => {
            FinishReason::ToolCall
        }
        other => FinishReason::Other(other.to_string()),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerateContentRequest {
    contents: Vec<GeminiRequestContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiRequestContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiRequestGenerationConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequestContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiRequestPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum GeminiRequestPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    FileData {
        #[serde(rename = "fileData")]
        file_data: GeminiFileData,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequestGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_modalities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    usage_metadata: Option<GeminiUsageMetadata>,
    model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GeminiResponsePart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    Other(Value),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiFileData {
    mime_type: String,
    file_uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: Option<u32>,
    candidates_token_count: Option<u32>,
    total_token_count: Option<u32>,
}

#[derive(Debug)]
enum HttpRequestFailure {
    Retryable(Error),
    Fatal(Error),
}

impl HttpRequestFailure {
    fn into_error(self) -> Error {
        match self {
            Self::Retryable(error) | Self::Fatal(error) => error,
        }
    }
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

fn guess_mime_type_from_url(url: &str) -> String {
    let lower = url.to_ascii_lowercase();

    if lower.ends_with(".png") {
        "image/png".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else if lower.ends_with(".gif") {
        "image/gif".to_string()
    } else {
        "image/jpeg".to_string()
    }
}
