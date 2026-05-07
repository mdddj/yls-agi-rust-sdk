use crate::{
    error::{Error, Result},
    provider::{AuthMode, HttpClientConfig, ProviderConfig},
    types::{ChatGptImageRequest, ChatGptImageResponse, ChatGptReferenceImage, GeneratedImage},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde_json::{Map, Value, json};
use url::Url;

const DEFAULT_API_KEY_ENV: &str = "YLS_CODEX_KEY";

#[derive(Debug, Clone)]
pub struct ChatGptImageClient {
    config: ProviderConfig,
}

impl ChatGptImageClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url_and_auth(
            api_key,
            Url::parse("https://code.ylsagi.com/codex/")?,
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

    pub fn with_config_and_client(
        api_key: impl Into<String>,
        base_url: Url,
        auth_mode: AuthMode,
        http_client: reqwest::Client,
    ) -> Result<Self> {
        Ok(Self {
            config: ProviderConfig {
                base_url,
                api_key: api_key.into(),
                auth_mode,
                http_client,
            },
        })
    }

    pub async fn generate_image(
        &self,
        request: ChatGptImageRequest,
    ) -> Result<ChatGptImageResponse> {
        let body = Self::build_request_body(&request);
        let response = self
            .config
            .auth_mode
            .apply(
                self.config
                    .http_client
                    .post(self.build_responses_url()?)
                    .json(&body),
                &self.config.api_key,
            )
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let response_body = response.text().await.map_err(|err| {
                Error::provider(
                    "chatgpt_image",
                    format!("failed to read error body (status {status}): {err}"),
                )
            })?;
            return Err(Error::provider(
                "chatgpt_image",
                format!(
                    "request failed with status {status}: {}",
                    truncate_error_body(&response_body)
                ),
            ));
        }

        let raw = collect_terminal_payload(response).await?;
        let image_result = extract_image_result(&raw).ok_or_else(|| {
            Error::provider(
                "chatgpt_image",
                "No image_generation_call result found in streamed SSE events.",
            )
        })?;
        let bytes = decode_image_result(image_result)?;
        let mime_type = detect_image_media_type(&bytes);

        Ok(ChatGptImageResponse {
            model: raw
                .get("response")
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            image: GeneratedImage {
                mime_type: mime_type.to_string(),
                data_base64: normalize_image_result(image_result),
                bytes,
                saved_info: None,
            },
            raw: Some(raw),
        })
    }

    fn build_request_body(request: &ChatGptImageRequest) -> Value {
        let input = if request.references.is_empty() {
            Value::String(request.prompt.clone())
        } else {
            let mut content = vec![json!({
                "type": "input_text",
                "text": request.prompt,
            })];

            for reference in &request.references {
                match reference {
                    ChatGptReferenceImage::FileId(file_id) => content.push(json!({
                        "type": "input_image",
                        "file_id": file_id,
                    })),
                    ChatGptReferenceImage::Url(url) => content.push(json!({
                        "type": "input_image",
                        "image_url": url,
                    })),
                }
            }

            Value::Array(vec![json!({
                "role": "user",
                "content": content,
            })])
        };

        let mut tool = Map::new();
        tool.insert(
            "type".to_string(),
            Value::String("image_generation".to_string()),
        );
        tool.insert(
            "model".to_string(),
            Value::String(request.image_model.clone()),
        );

        if let Some(tool_overrides) = &request.tool_overrides {
            for (key, value) in tool_overrides {
                tool.insert(key.clone(), value.clone());
            }
        }

        json!({
            "model": request.model,
            "input": input,
            "stream": true,
            "tools": [Value::Object(tool)],
        })
    }

    fn build_responses_url(&self) -> Result<Url> {
        let base = self.config.base_url.as_str();
        let normalized = if base.ends_with('/') {
            base.to_string()
        } else {
            format!("{base}/")
        };
        Ok(Url::parse(&normalized)?.join("responses")?)
    }
}

impl Default for ChatGptImageClient {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!("failed to build default ChatGptImageClient from {DEFAULT_API_KEY_ENV}: {err}")
        })
    }
}

#[deprecated(note = "use ChatGptImageClient instead")]
pub type ResponsesClient = ChatGptImageClient;

async fn collect_terminal_payload(response: reqwest::Response) -> Result<Value> {
    let mut stream = response.bytes_stream().eventsource();
    let mut final_payload = None;

    while let Some(event) = stream.next().await {
        let event =
            event.map_err(|err| Error::Stream(format!("failed to parse SSE stream: {err}")))?;
        let Some(payload) = parse_event_payload(&event.data) else {
            continue;
        };

        if let Some(error_payload) = extract_gateway_error(&payload) {
            return Err(Error::provider(
                "chatgpt_image",
                format_gateway_error_message(error_payload),
            ));
        }

        if extract_image_result(&payload).is_some() {
            final_payload = Some(payload);
            break;
        }
    }

    final_payload.ok_or_else(|| {
        Error::provider(
            "chatgpt_image",
            "No image_generation_call result found in streamed SSE events.",
        )
    })
}

fn parse_event_payload(data: &str) -> Option<Value> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn extract_gateway_error(payload: &Value) -> Option<&Value> {
    match payload.get("type").and_then(Value::as_str) {
        Some("error") => payload.get("error"),
        Some("response.failed") => payload
            .get("response")
            .and_then(|response| response.get("error")),
        _ => None,
    }
}

fn extract_image_result(payload: &Value) -> Option<&str> {
    let item = payload.get("item")?;
    let event_type = payload.get("type").and_then(Value::as_str)?;
    let item_type = item.get("type").and_then(Value::as_str)?;
    let result = item.get("result").and_then(Value::as_str)?;

    if event_type == "response.output_item.done"
        && item_type == "image_generation_call"
        && !result.trim().is_empty()
    {
        Some(result)
    } else {
        None
    }
}

fn normalize_image_result(result: &str) -> String {
    let trimmed = result.trim();
    if let Some((_, data)) = trimmed.rsplit_once(',') {
        if trimmed
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
        {
            return data.to_string();
        }
    }
    trimmed.to_string()
}

fn decode_image_result(result: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(normalize_image_result(result))
        .map_err(Error::from)
}

fn detect_image_media_type(buffer: &[u8]) -> &'static str {
    if buffer.len() >= 8
        && buffer[0] == 0x89
        && buffer[1] == 0x50
        && buffer[2] == 0x4e
        && buffer[3] == 0x47
        && buffer[4] == 0x0d
        && buffer[5] == 0x0a
        && buffer[6] == 0x1a
        && buffer[7] == 0x0a
    {
        return "image/png";
    }

    if buffer.len() >= 3 && buffer[0] == 0xff && buffer[1] == 0xd8 && buffer[2] == 0xff {
        return "image/jpeg";
    }

    if buffer.len() >= 12 && &buffer[0..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        return "image/webp";
    }

    if buffer.len() >= 6 && (&buffer[0..6] == b"GIF87a" || &buffer[0..6] == b"GIF89a") {
        return "image/gif";
    }

    "application/octet-stream"
}

fn format_gateway_error_message(error_payload: &Value) -> String {
    let error_type = error_payload
        .get("type")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown_error");
    let error_code = error_payload
        .get("code")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown_code");
    let error_message = error_payload
        .get("message")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("No error message returned by the gateway.");

    let mut lines = vec![format!(
        "Gateway image generation failed ({error_type}/{error_code}): {error_message}"
    )];

    if error_code == "moderation_blocked" || error_type == "image_generation_user_error" {
        lines.push("说明: 这类拦截通常发生在真实人物的照片级生成请求上。".to_string());
        lines.push(
            "建议: 不要直接使用真实人物姓名，改成泛化描述，并保留服装、场景、构图和灯光。"
                .to_string(),
        );
        lines.push(
            "参考改写: \"Photorealistic event photo of a glamorous actress with striking features, wearing a black leather stage outfit on a green-and-black GPU keynote stage, holding a graphics card, confident presentation pose, cinematic lighting, ultra detailed.\"".to_string(),
        );
    }

    lines.join("\n")
}

fn truncate_error_body(body: &str) -> String {
    const MAX_LEN: usize = 512;
    if body.len() <= MAX_LEN {
        body.to_string()
    } else {
        format!("{}...", &body[..MAX_LEN])
    }
}
