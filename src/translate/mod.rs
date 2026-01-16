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
        // 验证输入
        if text.trim().is_empty() {
            anyhow::bail!("Cannot translate empty text");
        }

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

        let (system_prompt, user_prompt) = build_translation_prompts(&self.config, request);

        let openai_req = OpenAIRequest {
            model: provider.model.clone(),
            messages: vec![
                OpenAIMessage { role: "system".to_string(), content: system_prompt },
                OpenAIMessage { role: "user".to_string(), content: user_prompt },
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

        let (system_prompt, user_prompt) = build_translation_prompts(&self.config, request);

        let anthropic_req = AnthropicRequest {
            model: provider.model.clone(),
            max_tokens: 4096,
            system: system_prompt,
            messages: vec![AnthropicMessage { role: "user".to_string(), content: user_prompt }],
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

fn get_language_name(code: &str) -> String {
    match code.to_lowercase().as_str() {
        "zh" | "zh-cn" => "简体中文".to_string(),
        "zh-tw" | "zh-hk" => "繁體中文".to_string(),
        "en" => "English".to_string(),
        "ja" => "日本語".to_string(),
        "ko" => "한국어".to_string(),
        "fr" => "Français".to_string(),
        "de" => "Deutsch".to_string(),
        "es" => "Español".to_string(),
        "ru" => "Русский".to_string(),
        "pt" => "Português".to_string(),
        "it" => "Italiano".to_string(),
        "ar" => "العربية".to_string(),
        "th" => "ไทย".to_string(),
        "vi" => "Tiếng Việt".to_string(),
        _ => code.to_string(), // 未知语言代码直接返回原值
    }
}

struct PromptTemplateContext<'a> {
    target_lang_code: &'a str,
    target_lang_name: String,
    source_lang_code: Option<&'a str>,
    text: &'a str,
}

fn render_prompt_template(template: &str, ctx: &PromptTemplateContext<'_>) -> String {
    let mut out = template.to_string();
    out = out.replace("{{target_lang_name}}", &ctx.target_lang_name);
    out = out.replace("{{target_lang_code}}", ctx.target_lang_code);
    out = out.replace("{{text}}", ctx.text);
    out = out.replace("{{source_lang_code}}", ctx.source_lang_code.unwrap_or_default());
    out
}

fn build_translation_prompts(config: &Config, request: &TranslateRequest) -> (String, String) {
    let ctx = PromptTemplateContext {
        target_lang_code: &request.target_lang,
        target_lang_name: get_language_name(&request.target_lang),
        source_lang_code: request.source_lang.as_deref(),
        text: &request.text,
    };

    let Some(preset) = config.active_prompt_preset() else {
        return (
            get_translation_system_prompt(&request.target_lang),
            get_translation_user_prompt(&request.target_lang, &request.text),
        );
    };

    let system = if preset.system_template.trim().is_empty() {
        get_translation_system_prompt(&request.target_lang)
    } else {
        render_prompt_template(&preset.system_template, &ctx)
    };

    let user = if preset.user_template.trim().is_empty() {
        get_translation_user_prompt(&request.target_lang, &request.text)
    } else {
        render_prompt_template(&preset.user_template, &ctx)
    };

    (system, user)
}

/// 生成翻译系统提示词
fn get_translation_system_prompt(target_lang: &str) -> String {
    let lang_name = get_language_name(target_lang);
    format!(
        r#"你是一位专业的 {} 母语翻译者，需要流畅地将文本翻译成 {}。

## 翻译规则
1. 仅输出翻译内容，不要包含解释或其他额外内容（例如"翻译如下："或"以下是翻译："等）
2. 返回的翻译必须保持与原文完全相同的段落数和格式
3. 如果文本包含 HTML 标签，在保持流畅性的同时，请考虑标签在翻译中的位置
4. 对于不应翻译的内容（如专有名词、代码等），请保留原文
5. 直接输出翻译（无分隔符，无额外文本）"#,
        lang_name, lang_name
    )
}

/// 生成翻译用户提示词
fn get_translation_user_prompt(target_lang: &str, text: &str) -> String {
    let lang_name = get_language_name(target_lang);
    format!("翻译成 {}（仅输出翻译）：\n\n{}", lang_name, text)
}
