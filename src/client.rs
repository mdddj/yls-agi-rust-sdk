use crate::{
    error::{Error, Result},
    provider::{AuthMode, ChatStream, ClaudeClient, GeminiClient, OpenAiClient, Provider},
    types::{ChatRequest, ChatResponse},
};
use std::sync::Arc;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";

#[derive(Clone)]
pub struct Client {
    openai: Arc<OpenAiClient>,
    gemini: Arc<GeminiClient>,
    claude: Arc<ClaudeClient>,
}

impl Client {
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(api_key)
    }

    pub fn from_env() -> Result<Self> {
        ClientBuilder::from_env()?.build()
    }

    pub async fn chat(&self, provider: Provider, request: ChatRequest) -> Result<ChatResponse> {
        match provider {
            Provider::OpenAi => self.openai.chat(request).await,
            Provider::Gemini => self.gemini.chat(request).await,
            Provider::Claude => self.claude.chat(request).await,
        }
    }

    pub async fn chat_stream(
        &self,
        provider: Provider,
        request: ChatRequest,
    ) -> Result<ChatStream> {
        match provider {
            Provider::OpenAi => self.openai.chat_stream(request).await,
            Provider::Gemini => self.gemini.chat_stream(request).await,
            Provider::Claude => self.claude.chat_stream(request).await,
        }
    }

    pub fn openai(&self) -> &OpenAiClient {
        &self.openai
    }

    pub fn gemini(&self) -> &GeminiClient {
        &self.gemini
    }

    pub fn claude(&self) -> &ClaudeClient {
        &self.claude
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!("failed to build default Client from {DEFAULT_API_KEY_ENV}: {err}")
        })
    }
}

pub struct ClientBuilder {
    api_key: String,
    openai_auth_mode: AuthMode,
    gemini_auth_mode: AuthMode,
    claude_auth_mode: AuthMode,
    openai_base_url: Option<String>,
    gemini_base_url: Option<String>,
    claude_base_url: Option<String>,
}

impl ClientBuilder {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            openai_auth_mode: AuthMode::AuthorizationBearer,
            gemini_auth_mode: AuthMode::XGoogApiKey,
            claude_auth_mode: AuthMode::AuthorizationKey,
            openai_base_url: None,
            gemini_base_url: None,
            claude_base_url: None,
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(DEFAULT_API_KEY_ENV)
            .map_err(|_| Error::MissingEnvVar(DEFAULT_API_KEY_ENV))?;
        Ok(Self::new(api_key))
    }

    pub fn with_openai_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.openai_auth_mode = auth_mode;
        self
    }

    pub fn with_gemini_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.gemini_auth_mode = auth_mode;
        self
    }

    pub fn with_claude_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.claude_auth_mode = auth_mode;
        self
    }

    pub fn with_openai_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.openai_base_url = Some(base_url.into());
        self
    }

    pub fn with_gemini_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.gemini_base_url = Some(base_url.into());
        self
    }

    pub fn with_claude_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.claude_base_url = Some(base_url.into());
        self
    }

    pub fn build(self) -> Result<Client> {
        let openai = OpenAiClient::with_base_url_and_auth(
            self.api_key.clone(),
            url::Url::parse(
                self.openai_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/openai/v1/"),
            )?,
            self.openai_auth_mode,
        )?;
        let gemini = GeminiClient::with_base_url_and_auth(
            self.api_key.clone(),
            url::Url::parse(
                self.gemini_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/gemini/v1beta/"),
            )?,
            self.gemini_auth_mode,
        )?;
        let claude = ClaudeClient::with_base_url_and_auth(
            self.api_key,
            url::Url::parse(
                self.claude_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/claude/v1/"),
            )?,
            self.claude_auth_mode,
        )?;

        Ok(Client {
            openai: Arc::new(openai),
            gemini: Arc::new(gemini),
            claude: Arc::new(claude),
        })
    }
}
