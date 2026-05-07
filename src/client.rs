use crate::{
    error::{Error, Result},
    provider::{
        AuthMode, ChatGptImageClient, ChatStream, ClaudeClient, GeminiClient, HttpClientConfig,
        OpenAiClient, Provider, ProxyConfig,
    },
    types::{ChatGptImageRequest, ChatGptImageResponse, ChatRequest, ChatResponse},
};
use std::sync::Arc;

const DEFAULT_API_KEY_ENV: &str = "YLS_AGI_KEY";
const DEFAULT_CHATGPT_IMAGE_API_KEY_ENV: &str = "YLS_CODEX_KEY";

#[derive(Clone)]
pub struct Client {
    openai: Arc<OpenAiClient>,
    gemini: Arc<GeminiClient>,
    claude: Arc<ClaudeClient>,
    chatgpt_image: Arc<ChatGptImageClient>,
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

    pub async fn generate_chatgpt_image(
        &self,
        request: ChatGptImageRequest,
    ) -> Result<ChatGptImageResponse> {
        self.chatgpt_image.generate_image(request).await
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

    pub fn chatgpt_image(&self) -> &ChatGptImageClient {
        &self.chatgpt_image
    }

    #[deprecated(note = "use generate_chatgpt_image instead")]
    pub async fn generate_image_via_responses(
        &self,
        request: ChatGptImageRequest,
    ) -> Result<ChatGptImageResponse> {
        self.generate_chatgpt_image(request).await
    }

    #[deprecated(note = "use chatgpt_image instead")]
    pub fn responses(&self) -> &ChatGptImageClient {
        self.chatgpt_image()
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|err| {
            panic!(
                "failed to build default Client from {DEFAULT_API_KEY_ENV}/{DEFAULT_CHATGPT_IMAGE_API_KEY_ENV}: {err}"
            )
        })
    }
}

pub struct ClientBuilder {
    api_key: String,
    chatgpt_image_api_key: Option<String>,
    openai_auth_mode: AuthMode,
    gemini_auth_mode: AuthMode,
    claude_auth_mode: AuthMode,
    chatgpt_image_auth_mode: AuthMode,
    openai_base_url: Option<String>,
    gemini_base_url: Option<String>,
    claude_base_url: Option<String>,
    chatgpt_image_base_url: Option<String>,
    http_client_config: HttpClientConfig,
}

impl ClientBuilder {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            chatgpt_image_api_key: None,
            openai_auth_mode: AuthMode::AuthorizationBearer,
            gemini_auth_mode: AuthMode::XGoogApiKey,
            claude_auth_mode: AuthMode::AuthorizationKey,
            chatgpt_image_auth_mode: AuthMode::AuthorizationBearer,
            openai_base_url: None,
            gemini_base_url: None,
            claude_base_url: None,
            chatgpt_image_base_url: None,
            http_client_config: HttpClientConfig::default(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(DEFAULT_API_KEY_ENV)
            .map_err(|_| Error::MissingEnvVar(DEFAULT_API_KEY_ENV))?;
        let chatgpt_image_api_key = std::env::var(DEFAULT_CHATGPT_IMAGE_API_KEY_ENV)
            .map_err(|_| Error::MissingEnvVar(DEFAULT_CHATGPT_IMAGE_API_KEY_ENV))?;

        Ok(Self::new(api_key).with_chatgpt_image_api_key(chatgpt_image_api_key))
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

    pub fn with_chatgpt_image_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.chatgpt_image_api_key = Some(api_key.into());
        self
    }

    pub fn with_chatgpt_image_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.chatgpt_image_auth_mode = auth_mode;
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

    pub fn with_chatgpt_image_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.chatgpt_image_base_url = Some(base_url.into());
        self
    }

    #[deprecated(note = "use with_chatgpt_image_auth_mode instead")]
    pub fn with_responses_auth_mode(self, auth_mode: AuthMode) -> Self {
        self.with_chatgpt_image_auth_mode(auth_mode)
    }

    #[deprecated(note = "use with_chatgpt_image_base_url instead")]
    pub fn with_responses_base_url(self, base_url: impl Into<String>) -> Self {
        self.with_chatgpt_image_base_url(base_url)
    }

    pub fn with_proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.http_client_config.proxy = Some(ProxyConfig::Custom(proxy_url.into()));
        self
    }

    pub fn without_proxy(mut self) -> Self {
        self.http_client_config.proxy = Some(ProxyConfig::Disable);
        self
    }

    pub fn with_system_proxy(mut self) -> Self {
        self.http_client_config.proxy = Some(ProxyConfig::UseSystem);
        self
    }

    pub fn build(self) -> Result<Client> {
        let chatgpt_image_api_key = self
            .chatgpt_image_api_key
            .clone()
            .unwrap_or_else(|| self.api_key.clone());
        let openai = OpenAiClient::with_config(
            self.api_key.clone(),
            url::Url::parse(
                self.openai_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/openai/v1/"),
            )?,
            self.openai_auth_mode,
            self.http_client_config.clone(),
        )?;
        let gemini = GeminiClient::with_config(
            self.api_key.clone(),
            url::Url::parse(
                self.gemini_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/gemini/v1beta/"),
            )?,
            self.gemini_auth_mode,
            self.http_client_config.clone(),
        )?;
        let claude = ClaudeClient::with_config(
            self.api_key.clone(),
            url::Url::parse(
                self.claude_base_url
                    .as_deref()
                    .unwrap_or("https://api.ylsagi.com/claude/v1/"),
            )?,
            self.claude_auth_mode,
            self.http_client_config.clone(),
        )?;
        let chatgpt_image = ChatGptImageClient::with_config(
            chatgpt_image_api_key,
            url::Url::parse(
                self.chatgpt_image_base_url
                    .as_deref()
                    .unwrap_or("https://code.ylsagi.com/codex/"),
            )?,
            self.chatgpt_image_auth_mode,
            self.http_client_config,
        )?;

        Ok(Client {
            openai: Arc::new(openai),
            gemini: Arc::new(gemini),
            claude: Arc::new(claude),
            chatgpt_image: Arc::new(chatgpt_image),
        })
    }
}
