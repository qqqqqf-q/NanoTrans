//! Configuration management
//! Handles loading, saving, and managing application settings

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Provider types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Google,     // Free, no config needed
    DeepL,      // Needs API key only
    OpenAI,     // OpenAI-compatible API
    Anthropic,  // Anthropic API
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub is_preset: bool,
}

/// Prompt preset for LLM translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPreset {
    pub id: String,
    pub name: String,
    pub system_template: String,
    pub user_template: String,
    #[serde(default)]
    pub is_preset: bool,
}

/// UI language
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UILanguage {
    #[default]
    Auto,   // 跟随系统
    En,     // English
    Zh,     // 中文
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: String,
    #[serde(default)]
    pub hotkey_log_enabled: bool,
    pub target_lang: String,
    pub source_lang: String,
    pub auto_detect: bool,
    pub active_provider_id: String,
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_active_prompt_preset_id")]
    pub active_prompt_preset_id: String,
    #[serde(default = "default_prompt_presets")]
    pub prompt_presets: Vec<PromptPreset>,
    #[serde(default)]
    pub ui_language: UILanguage,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Q".to_string(),
            hotkey_log_enabled: false,
            target_lang: "zh".to_string(),
            source_lang: String::new(),
            auto_detect: true,
            active_provider_id: "google".to_string(),
            providers: default_providers(),
            active_prompt_preset_id: default_active_prompt_preset_id(),
            prompt_presets: default_prompt_presets(),
            ui_language: UILanguage::Auto,
        }
    }
}

fn default_active_prompt_preset_id() -> String {
    "default".to_string()
}

fn default_prompt_presets() -> Vec<PromptPreset> {
    vec![
        PromptPreset {
            id: "default".to_string(),
            name: "默认（严格）".to_string(),
            system_template: r#"你是一位专业的 {{target_lang_name}} 母语翻译者，需要流畅地将文本翻译成 {{target_lang_name}}。

## 翻译规则
1. 仅输出翻译内容，不要包含解释或其他额外内容（例如"翻译如下："或"以下是翻译："等）
2. 返回的翻译必须保持与原文完全相同的段落数和格式
3. 如果文本包含 HTML 标签，在保持流畅性的同时，请考虑标签在翻译中的位置
4. 对于不应翻译的内容（如专有名词、代码等），请保留原文
5. 直接输出翻译（无分隔符，无额外文本）"#.to_string(),
            user_template: "翻译成 {{target_lang_name}}（仅输出翻译）：\n\n{{text}}".to_string(),
            is_preset: true,
        },
        PromptPreset {
            id: "polish".to_string(),
            name: "更自然（轻润色）".to_string(),
            system_template: r#"你是一位专业的 {{target_lang_name}} 母语译者。请在忠实原意的前提下，让译文更自然、更符合目标语言的表达习惯。

规则：
1. 仅输出译文，不要附加解释、标题或标注
2. 段落与格式保持一致（包括换行、列表等）
3. 遇到代码、专有名词、链接等不应翻译内容时，保持原样"#.to_string(),
            user_template: "将下文翻译为 {{target_lang_name}}：\n\n{{text}}".to_string(),
            is_preset: true,
        },
    ]
}

/// Get default provider presets
fn default_providers() -> Vec<ProviderConfig> {
    vec![
        // Google Translate - Free, no config
        ProviderConfig {
            id: "google".to_string(),
            name: "Google Translate".to_string(),
            provider_type: ProviderType::Google,
            api_base: String::new(),
            api_key: String::new(),
            model: String::new(),
            is_preset: true,
        },
        // DeepL - Needs API key
        ProviderConfig {
            id: "deepl".to_string(),
            name: "DeepL".to_string(),
            provider_type: ProviderType::DeepL,
            api_base: "https://api-free.deepl.com/v2".to_string(),
            api_key: String::new(),
            model: String::new(),
            is_preset: true,
        },
        // Zhipu GLM
        ProviderConfig {
            id: "zhipu".to_string(),
            name: "Zhipu GLM".to_string(),
            provider_type: ProviderType::OpenAI,
            api_base: "https://open.bigmodel.cn/api/paas/v4".to_string(),
            api_key: String::new(),
            model: "glm-4-flash".to_string(),
            is_preset: true,
        },
        // OpenAI
        ProviderConfig {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            provider_type: ProviderType::OpenAI,
            api_base: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            is_preset: true,
        },
        // Anthropic
        ProviderConfig {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            provider_type: ProviderType::Anthropic,
            api_base: "https://api.anthropic.com".to_string(),
            api_key: String::new(),
            model: "claude-3-5-haiku-latest".to_string(),
            is_preset: true,
        },
        // Custom OpenAI-compatible
        ProviderConfig {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            provider_type: ProviderType::OpenAI,
            api_base: String::new(),
            api_key: String::new(),
            model: String::new(),
            is_preset: false,
        },
    ]
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
            .join("NanoTrans");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }
        Ok(config_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: Config = serde_json::from_str(&content)?;
            config.normalize();
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == self.active_provider_id)
    }

    pub fn active_provider_mut(&mut self) -> Option<&mut ProviderConfig> {
        self.providers.iter_mut().find(|p| p.id == self.active_provider_id)
    }

    pub fn get_provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn get_provider_mut(&mut self, id: &str) -> Option<&mut ProviderConfig> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    pub fn provider_index(&self, id: &str) -> Option<usize> {
        self.providers.iter().position(|p| p.id == id)
    }

    pub fn prompt_preset_index(&self, id: &str) -> Option<usize> {
        self.prompt_presets.iter().position(|p| p.id == id)
    }

    pub fn active_prompt_preset(&self) -> Option<&PromptPreset> {
        self.prompt_presets.iter().find(|p| p.id == self.active_prompt_preset_id)
    }

    pub fn active_prompt_preset_mut(&mut self) -> Option<&mut PromptPreset> {
        self.prompt_presets.iter_mut().find(|p| p.id == self.active_prompt_preset_id)
    }

    pub fn get_prompt_preset(&self, id: &str) -> Option<&PromptPreset> {
        self.prompt_presets.iter().find(|p| p.id == id)
    }

    pub fn get_prompt_preset_mut(&mut self, id: &str) -> Option<&mut PromptPreset> {
        self.prompt_presets.iter_mut().find(|p| p.id == id)
    }

    pub fn normalize(&mut self) {
        self.normalize_providers();
        if self.prompt_presets.is_empty() {
            self.prompt_presets = default_prompt_presets();
        }
        if self.prompt_preset_index(&self.active_prompt_preset_id).is_none() {
            self.active_prompt_preset_id = self
                .prompt_presets
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(default_active_prompt_preset_id);
        }
    }

    fn normalize_providers(&mut self) {
        let defaults = default_providers();
        if self.providers.is_empty() {
            self.providers = defaults;
        } else {
            let mut existing: HashMap<String, ProviderConfig> = self
                .providers
                .drain(..)
                .map(|p| (p.id.clone(), p))
                .collect();
            let mut merged = Vec::with_capacity(defaults.len() + existing.len());
            for def in defaults {
                if let Some(mut saved) = existing.remove(&def.id) {
                    saved.name = def.name;
                    saved.provider_type = def.provider_type;
                    saved.is_preset = def.is_preset;
                    if saved.api_base.trim().is_empty() {
                        saved.api_base = def.api_base;
                    }
                    if saved.model.trim().is_empty() {
                        saved.model = def.model;
                    }
                    merged.push(saved);
                } else {
                    merged.push(def);
                }
            }
            if !existing.is_empty() {
                let mut extras: Vec<ProviderConfig> = existing.into_values().collect();
                extras.sort_by(|a, b| a.id.cmp(&b.id));
                merged.extend(extras);
            }
            self.providers = merged;
        }

        // 防止无关字段被写进不需要配置的服务里
        for provider in &mut self.providers {
            match provider.provider_type {
                ProviderType::Google => {
                    provider.api_base.clear();
                    provider.api_key.clear();
                    provider.model.clear();
                }
                ProviderType::DeepL => {
                    provider.model.clear();
                }
                ProviderType::OpenAI | ProviderType::Anthropic => {}
            }
        }

        if self.provider_index(&self.active_provider_id).is_none() {
            self.active_provider_id = self
                .providers
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| "google".to_string());
        }
    }
}
