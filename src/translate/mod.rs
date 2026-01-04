//! Translation module
//! Supports multiple translation providers with unified configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{Config, ProviderConfig, ProviderType};

/// Translation request
#[derive(Debug, Clone)]
pub struct TranslateRequest {
    pub text: String,
    pub source_lang: Option<String>,
    pub target_lang: String,
}

/// Translation response
#[derive(Debug, Clone)]
pub struct TranslateResponse {
    pub translated_text: String,
}

/// Main translator that dispatches to the configured provider
pub struct Translator {
    config: Config,
    client: reqwest::Client,
}

impl Translator {
    pub fn new(config: Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self { config, client }
    }

    /// Translate text using the active provider
    pub async fn translate(&self, text: &str) -> Result<TranslateResponse> {
        let provider = self.config.active_provider()
            .ok_or_else(|| anyhow::anyhow!("No active provider configured"))?;

        let request = TranslateRequest {
            text: text.to_string(),
            source_lang: if self.config.auto_detect { None } else { Some(self.config.source_lang.clone()) },
            target_lang: self.determine_target_lang(text),
        };

        match provider.provider_type {
            ProviderType::Google => self.translate_google(&request).await,
            ProviderType::DeepL => self.translate_deepl(provider, &request).await,
            ProviderType::OpenAI => self.translate_openai(provider, &request).await,
            ProviderType::Anthropic => self.translate_anthropic(provider, &request).await,
        }
    }

    /// Determine target language based on source text
    fn determine_target_lang(&self, text: &str) -> String {
        if self.config.auto_detect {
            let has_cjk = text.chars().any(|c| {
                matches!(c,
                    '\u{4E00}'..='\u{9FFF}' |
                    '\u{3400}'..='\u{4DBF}' |
                    '\u{3040}'..='\u{309F}' |
                    '\u{30A0}'..='\u{30FF}'
                )
            });
            if has_cjk { "en".to_string() } else { "zh".to_string() }
        } else {
            self.config.target_lang.clone()
        }
    }

    /// Google Translate (free, no API key needed)
    async fn translate_google(&self, request: &TranslateRequest) -> Result<TranslateResponse> {
        let source = request.source_lang.as_deref().unwrap_or("auto");
        let encoded_text = urlencoding::encode(&request.text);

        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            source, request.target_lang, encoded_text
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?
            .text()
            .await?;

        let parsed: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse Google response: {}", e))?;

        let mut translated_text = String::new();
        if let Some(outer_array) = parsed.get(0).and_then(|v| v.as_array()) {
            for item in outer_array {
                if let Some(text_part) = item.get(0).and_then(|v| v.as_str()) {
                    translated_text.push_str(text_part);
                }
            }
        }

        if translated_text.is_empty() {
            anyhow::bail!("No translation returned from Google");
        }

        Ok(TranslateResponse { translated_text })
    }

    /// DeepL translation
    async fn translate_deepl(&self, provider: &ProviderConfig, request: &TranslateRequest) -> Result<TranslateResponse> {
        if provider.api_key.is_empty() {
            anyhow::bail!("DeepL API key not configured");
        }

        #[derive(Serialize)]
        struct DeepLRequest {
            text: Vec<String>,
            target_lang: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            source_lang: Option<String>,
        }

        #[derive(Deserialize)]
        struct DeepLResponse {
            translations: Vec<DeepLTranslation>,
        }

        #[derive(Deserialize)]
        struct DeepLTranslation {
            text: String,
        }

        let deepl_req = DeepLRequest {
            text: vec![request.text.clone()],
            target_lang: request.target_lang.to_uppercase(),
            source_lang: request.source_lang.clone().map(|s| s.to_uppercase()),
        };

        let url = format!("{}/translate", provider.api_base.trim_end_matches('/'));

        let response = self.client
            .post(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", provider.api_key))
            .json(&deepl_req)
            .send()
            .await?
            .json::<DeepLResponse>()
            .await?;

        let translation = response.translations.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No translation returned from DeepL"))?;

        Ok(TranslateResponse { translated_text: translation.text })
    }

    /// OpenAI-compatible API translation
    async fn translate_openai(&self, provider: &ProviderConfig, request: &TranslateRequest) -> Result<TranslateResponse> {
        if provider.api_key.is_empty() {
            anyhow::bail!("{} API key not configured", provider.name);
        }

        #[derive(Serialize)]
        struct OpenAIRequest {
            model: String,
            messages: Vec<OpenAIMessage>,
            temperature: f32,
        }

        #[derive(Serialize)]
        struct OpenAIMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAIChoice {
            message: OpenAIMessageResponse,
        }

        #[derive(Deserialize)]
        struct OpenAIMessageResponse {
            content: String,
        }

        let system_prompt = format!(
            "You are a professional translator. Translate the following text to {}. Only output the translation, nothing else.",
            get_language_name(&request.target_lang)
        );

        let openai_req = OpenAIRequest {
            model: provider.model.clone(),
            messages: vec![
                OpenAIMessage { role: "system".to_string(), content: system_prompt },
                OpenAIMessage { role: "user".to_string(), content: request.text.clone() },
            ],
            temperature: 0.3,
        };

        let url = format!("{}/chat/completions", provider.api_base.trim_end_matches('/'));

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_req)
            .send()
            .await?
            .json::<OpenAIResponse>()
            .await?;

        let translation = response.choices.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No response from {}", provider.name))?
            .message.content;

        Ok(TranslateResponse { translated_text: translation.trim().to_string() })
    }

    /// Anthropic API translation
    async fn translate_anthropic(&self, provider: &ProviderConfig, request: &TranslateRequest) -> Result<TranslateResponse> {
        if provider.api_key.is_empty() {
            anyhow::bail!("Anthropic API key not configured");
        }

        #[derive(Serialize)]
        struct AnthropicRequest {
            model: String,
            max_tokens: u32,
            system: String,
            messages: Vec<AnthropicMessage>,
        }

        #[derive(Serialize)]
        struct AnthropicMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<AnthropicContent>,
        }

        #[derive(Deserialize)]
        struct AnthropicContent {
            text: String,
        }

        let system_prompt = format!(
            "You are a professional translator. Translate the following text to {}. Only output the translation, nothing else.",
            get_language_name(&request.target_lang)
        );

        let anthropic_req = AnthropicRequest {
            model: provider.model.clone(),
            max_tokens: 4096,
            system: system_prompt,
            messages: vec![AnthropicMessage { role: "user".to_string(), content: request.text.clone() }],
        };

        let url = format!("{}/v1/messages", provider.api_base.trim_end_matches('/'));

        let response = self.client
            .post(&url)
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_req)
            .send()
            .await?
            .json::<AnthropicResponse>()
            .await?;

        let translation = response.content.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No response from Anthropic"))?
            .text;

        Ok(TranslateResponse { translated_text: translation.trim().to_string() })
    }
}

fn get_language_name(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "zh" | "zh-cn" => "Chinese (Simplified)",
        "en" => "English",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "ru" => "Russian",
        _ => "English",
    }
}
