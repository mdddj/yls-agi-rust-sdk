use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAiModel {
    Gpt41,
    Gpt5Mini,
    Gpt51,
    Gpt51Chat,
    O4MiniDeepResearch,
    DeepseekV32Exp,
    Gpt52,
    Gpt52Chat,
    Gpt54,
}

impl OpenAiModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gpt41 => "gpt-4.1",
            Self::Gpt5Mini => "gpt-5-mini",
            Self::Gpt51 => "gpt-5.1",
            Self::Gpt51Chat => "gpt-5.1-chat",
            Self::O4MiniDeepResearch => "o4-mini-deep-research",
            Self::DeepseekV32Exp => "deepseek-v3.2-exp",
            Self::Gpt52 => "gpt-5.2",
            Self::Gpt52Chat => "gpt-5.2-chat",
            Self::Gpt54 => "gpt-5.4",
        }
    }
}

impl Display for OpenAiModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<OpenAiModel> for String {
    fn from(value: OpenAiModel) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaudeModel {
    ClaudeHaiku4520251001,
    ClaudeSonnet4520250929,
    ClaudeOpus4520251101,
    ClaudeSonnet46,
    ClaudeOpus46,
}

impl ClaudeModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeHaiku4520251001 => "claude-haiku-4-5-20251001",
            Self::ClaudeSonnet4520250929 => "claude-sonnet-4-5-20250929",
            Self::ClaudeOpus4520251101 => "claude-opus-4-5-20251101",
            Self::ClaudeSonnet46 => "claude-sonnet-4-6",
            Self::ClaudeOpus46 => "claude-opus-4-6",
        }
    }
}

impl Display for ClaudeModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ClaudeModel> for String {
    fn from(value: ClaudeModel) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeminiModel {
    Gemini3ProPreview,
    Gemini3FlashPreview,
    Gemini25FlashImage,
    Gemini3ProImagePreview,
    Gemini31ProPreview,
}

impl GeminiModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gemini3ProPreview => "gemini-3-pro-preview",
            Self::Gemini3FlashPreview => "gemini-3-flash-preview",
            Self::Gemini25FlashImage => "gemini-2.5-flash-image",
            Self::Gemini3ProImagePreview => "gemini-3-pro-image-preview",
            Self::Gemini31ProPreview => "gemini-3.1-pro-preview",
        }
    }

    pub const fn supports_image_generation(self) -> bool {
        matches!(
            self,
            Self::Gemini25FlashImage | Self::Gemini3ProImagePreview
        )
    }
}

impl Display for GeminiModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<GeminiModel> for String {
    fn from(value: GeminiModel) -> Self {
        value.as_str().to_string()
    }
}
