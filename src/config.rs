//! Configuration management
//! Handles loading, saving, and managing application settings

use anyhow::Result;
use serde::{Deserialize, Serialize};
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
    pub target_lang: String,
    pub source_lang: String,
    pub auto_detect: bool,
    pub active_provider_id: String,
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub ui_language: UILanguage,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Q".to_string(),
            target_lang: "zh".to_string(),
            source_lang: String::new(),
            auto_detect: true,
            active_provider_id: "google".to_string(),
            providers: default_providers(),
            ui_language: UILanguage::Auto,
        }
    }
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
            let config: Config = serde_json::from_str(&content)?;
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
}
