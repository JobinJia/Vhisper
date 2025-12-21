use serde::{Deserialize, Serialize};

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub hotkey: HotkeyConfig,
    #[serde(default)]
    pub asr: AsrConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: HotkeyConfig::default(),
            asr: AsrConfig::default(),
            llm: LlmConfig::default(),
            output: OutputConfig::default(),
        }
    }
}

/// 快捷键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_trigger_key")]
    pub trigger_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_trigger_key() -> String {
    "Alt".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            trigger_key: default_trigger_key(),
            enabled: true,
        }
    }
}

/// ASR 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    #[serde(default = "default_asr_provider")]
    pub provider: String,
    #[serde(default)]
    pub dashscope: Option<DashScopeAsrConfig>,
    #[serde(default)]
    pub qwen: Option<QwenAsrConfig>,
    #[serde(default)]
    pub openai: Option<OpenAiAsrConfig>,
    #[serde(default)]
    pub funasr: Option<FunAsrConfig>,
}

fn default_asr_provider() -> String {
    "Qwen".to_string()
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            provider: default_asr_provider(),
            dashscope: None,
            qwen: None,
            openai: None,
            funasr: None,
        }
    }
}

/// DashScope ASR 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashScopeAsrConfig {
    pub api_key: String,
    #[serde(default = "default_dashscope_model")]
    pub model: String,
}

fn default_dashscope_model() -> String {
    "paraformer-realtime-v2".to_string()
}

/// 通义千问 ASR 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenAsrConfig {
    pub api_key: String,
    #[serde(default = "default_qwen_asr_model")]
    pub model: String,
}

fn default_qwen_asr_model() -> String {
    "qwen3-asr-flash-realtime".to_string()
}

/// OpenAI ASR 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiAsrConfig {
    pub api_key: String,
    #[serde(default = "default_whisper_model")]
    pub model: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_whisper_model() -> String {
    "whisper-1".to_string()
}

fn default_language() -> String {
    "zh".to_string()
}

/// FunASR 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunAsrConfig {
    #[serde(default = "default_funasr_endpoint")]
    pub endpoint: String,
}

fn default_funasr_endpoint() -> String {
    "http://localhost:10096".to_string()
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default)]
    pub dashscope: Option<DashScopeLlmConfig>,
    #[serde(default)]
    pub openai: Option<OpenAiLlmConfig>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
}

fn default_llm_provider() -> String {
    "DashScope".to_string()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: default_llm_provider(),
            dashscope: None,
            openai: None,
            ollama: None,
        }
    }
}

/// DashScope LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashScopeLlmConfig {
    pub api_key: String,
    #[serde(default = "default_qwen_model")]
    pub model: String,
}

fn default_qwen_model() -> String {
    "qwen-plus".to_string()
}

/// OpenAI LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiLlmConfig {
    pub api_key: String,
    #[serde(default = "default_gpt_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_gpt_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_temperature() -> f32 {
    0.3
}

fn default_max_tokens() -> u32 {
    2000
}

/// Ollama 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

fn default_ollama_endpoint() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "qwen3:8b".to_string()
}

/// 输出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_true")]
    pub restore_clipboard: bool,
    #[serde(default = "default_paste_delay")]
    pub paste_delay_ms: u64,
}

fn default_paste_delay() -> u64 {
    50
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            restore_clipboard: true,
            paste_delay_ms: default_paste_delay(),
        }
    }
}
