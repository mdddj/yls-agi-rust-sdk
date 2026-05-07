use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMimeType {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
    Tiff,
}

impl ImageMimeType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
        }
    }
}

impl From<ImageMimeType> for String {
    fn from(value: ImageMimeType) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<MessagePart>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self::from_text(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::from_text(Role::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::from_text(Role::Assistant, content)
    }

    pub fn from_parts(role: Role, content: Vec<MessagePart>) -> Self {
        Self { role, content }
    }

    pub fn from_text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![MessagePart::text(content)],
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.content.push(MessagePart::text(text));
        self
    }

    pub fn with_image_url(mut self, url: impl Into<String>) -> Self {
        self.content.push(MessagePart::image_url(url));
        self
    }

    pub fn with_image_base64(
        mut self,
        mime_type: impl Into<ImageMime>,
        data_base64: impl Into<String>,
    ) -> Self {
        self.content
            .push(MessagePart::image_base64(mime_type, data_base64));
        self
    }

    pub fn with_image_bytes(
        mut self,
        mime_type: impl Into<ImageMime>,
        bytes: impl AsRef<[u8]>,
    ) -> Self {
        self.content
            .push(MessagePart::image_bytes(mime_type, bytes));
        self
    }

    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    ImageUrl {
        url: String,
    },
    ImageBase64 {
        mime_type: String,
        data_base64: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMime {
    Known(ImageMimeType),
    Custom(String),
}

impl ImageMime {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(mime_type) => mime_type.as_str(),
            Self::Custom(value) => value.as_str(),
        }
    }
}

impl From<ImageMimeType> for ImageMime {
    fn from(value: ImageMimeType) -> Self {
        Self::Known(value)
    }
}

impl From<String> for ImageMime {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

impl From<&str> for ImageMime {
    fn from(value: &str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl MessagePart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl { url: url.into() }
    }

    pub fn image_base64(mime_type: impl Into<ImageMime>, data_base64: impl Into<String>) -> Self {
        Self::ImageBase64 {
            mime_type: mime_type.into().as_str().to_string(),
            data_base64: data_base64.into(),
        }
    }

    pub fn image_bytes(mime_type: impl Into<ImageMime>, bytes: impl AsRef<[u8]>) -> Self {
        Self::ImageBase64 {
            mime_type: mime_type.into().as_str().to_string(),
            data_base64: BASE64.encode(bytes.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationOptions {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub options: GenerationOptions,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            stream: false,
            options: GenerationOptions::default(),
        }
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_options(mut self, options: GenerationOptions) -> Self {
        self.options = options;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    #[default]
    Stop,
    Length,
    ContentFilter,
    ToolCall,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub model: Option<String>,
    pub message: ChatMessage,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<Usage>,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatChunk {
    pub delta: String,
    pub done: bool,
    pub finish_reason: Option<FinishReason>,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiImageRequest {
    pub model: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub reference_images: Vec<GeminiReferenceImage>,
    pub options: GenerationOptions,
}

impl GeminiImageRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            system_prompt: None,
            reference_images: Vec::new(),
            options: GenerationOptions::default(),
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn with_reference_image(mut self, image: GeminiReferenceImage) -> Self {
        self.reference_images.push(image);
        self
    }

    pub fn with_reference_image_base64(
        mut self,
        mime_type: impl Into<ImageMime>,
        data_base64: impl Into<String>,
    ) -> Self {
        self.reference_images
            .push(GeminiReferenceImage::from_base64(mime_type, data_base64));
        self
    }

    pub fn with_reference_image_bytes(
        mut self,
        mime_type: impl Into<ImageMime>,
        bytes: impl AsRef<[u8]>,
    ) -> Self {
        self.reference_images
            .push(GeminiReferenceImage::from_bytes(mime_type, bytes));
        self
    }

    pub fn with_options(mut self, options: GenerationOptions) -> Self {
        self.options = options;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeminiReferenceImage {
    pub mime_type: String,
    pub data_base64: String,
}

impl GeminiReferenceImage {
    pub fn from_base64(mime_type: impl Into<ImageMime>, data_base64: impl Into<String>) -> Self {
        Self {
            mime_type: mime_type.into().as_str().to_string(),
            data_base64: data_base64.into(),
        }
    }

    pub fn from_bytes(mime_type: impl Into<ImageMime>, bytes: impl AsRef<[u8]>) -> Self {
        Self {
            mime_type: mime_type.into().as_str().to_string(),
            data_base64: BASE64.encode(bytes.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub mime_type: String,
    pub data_base64: String,
    pub bytes: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_info: Option<SavedImageInfo>,
}

impl GeneratedImage {
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::fs::write(path, &self.bytes)
    }

    pub fn save_with_metadata(
        &mut self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<SavedImageInfo> {
        let path = path.as_ref();
        std::fs::write(path, &self.bytes)?;
        let absolute_output_path =
            std::fs::canonicalize(path).unwrap_or_else(|_| absolute_path_fallback(path));
        let saved_info = SavedImageInfo {
            output_path: path.display().to_string(),
            absolute_output_path: absolute_output_path.display().to_string(),
            byte_length: self.bytes.len(),
        };
        self.saved_info = Some(saved_info.clone());
        Ok(saved_info)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedImageInfo {
    pub output_path: String,
    pub absolute_output_path: String,
    pub byte_length: usize,
}

pub fn format_duration_ms(total_duration_ms: u128) -> String {
    if total_duration_ms < 1_000 {
        return format!("{total_duration_ms} ms");
    }

    if total_duration_ms < 60_000 {
        let secs = total_duration_ms as f64 / 1_000.0;
        return format!("{secs:.2} s");
    }

    let total_seconds = total_duration_ms as f64 / 1_000.0;
    let minutes = (total_seconds / 60.0).floor() as u64;
    let seconds = total_seconds - (minutes as f64 * 60.0);
    format!("{minutes} min {seconds:.2} s")
}

fn absolute_path_fallback(path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeminiImageResponse {
    pub model: Option<String>,
    pub text: Vec<String>,
    pub images: Vec<GeneratedImage>,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatGptImageRequest {
    pub model: String,
    pub prompt: String,
    pub image_model: String,
    pub references: Vec<ChatGptReferenceImage>,
    pub tool_overrides: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ChatGptImageRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            image_model: "gpt-image-2".to_string(),
            references: Vec::new(),
            tool_overrides: None,
        }
    }

    pub fn with_image_model(mut self, image_model: impl Into<String>) -> Self {
        self.image_model = image_model.into();
        self
    }

    pub fn with_reference(mut self, reference: ChatGptReferenceImage) -> Self {
        self.references.push(reference);
        self
    }

    pub fn with_reference_url(mut self, url: impl Into<String>) -> Self {
        self.references.push(ChatGptReferenceImage::url(url));
        self
    }

    pub fn with_reference_file_id(mut self, file_id: impl Into<String>) -> Self {
        self.references
            .push(ChatGptReferenceImage::file_id(file_id));
        self
    }

    pub fn with_reference_base64(
        mut self,
        mime_type: impl Into<ImageMime>,
        data_base64: impl Into<String>,
    ) -> Self {
        self.references
            .push(ChatGptReferenceImage::from_base64(mime_type, data_base64));
        self
    }

    pub fn with_reference_bytes(
        mut self,
        mime_type: impl Into<ImageMime>,
        bytes: impl AsRef<[u8]>,
    ) -> Self {
        self.references
            .push(ChatGptReferenceImage::from_bytes(mime_type, bytes));
        self
    }

    pub fn with_tool_overrides(
        mut self,
        tool_overrides: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.tool_overrides = Some(tool_overrides);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatGptReferenceImage {
    Url(String),
    FileId(String),
}

impl ChatGptReferenceImage {
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

    pub fn file_id(file_id: impl Into<String>) -> Self {
        Self::FileId(file_id.into())
    }

    pub fn from_base64(mime_type: impl Into<ImageMime>, data_base64: impl Into<String>) -> Self {
        Self::Url(format!(
            "data:{};base64,{}",
            mime_type.into().as_str(),
            data_base64.into()
        ))
    }

    pub fn from_bytes(mime_type: impl Into<ImageMime>, bytes: impl AsRef<[u8]>) -> Self {
        Self::from_base64(mime_type, BASE64.encode(bytes.as_ref()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatGptImageResponse {
    pub model: Option<String>,
    pub image: GeneratedImage,
    pub raw: Option<serde_json::Value>,
}

#[deprecated(note = "use ChatGptImageRequest instead")]
pub type ResponsesImageRequest = ChatGptImageRequest;

#[deprecated(note = "use ChatGptReferenceImage instead")]
pub type ResponsesReferenceImage = ChatGptReferenceImage;

#[deprecated(note = "use ChatGptImageResponse instead")]
pub type ResponsesImageResponse = ChatGptImageResponse;
