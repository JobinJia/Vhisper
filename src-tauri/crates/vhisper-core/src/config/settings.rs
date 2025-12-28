use serde::{Deserialize, Serialize};

/// 键码枚举 - 支持所有常用键
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum KeyCode {
    // 修饰键
    Alt,
    Control,
    Shift,
    Meta, // Cmd on macOS, Win on Windows

    // 功能键
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // 字母键
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    // 数字键
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // 特殊键
    Space,
    Tab,
    CapsLock,
    Escape,
    Backquote, // `
}

impl Default for KeyCode {
    fn default() -> Self {
        KeyCode::Alt
    }
}

impl KeyCode {
    /// 判断是否是修饰键
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            KeyCode::Alt | KeyCode::Control | KeyCode::Shift | KeyCode::Meta
        )
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            KeyCode::Alt => "Alt",
            KeyCode::Control => "Control",
            KeyCode::Shift => "Shift",
            KeyCode::Meta => "Meta",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::KeyA => "A",
            KeyCode::KeyB => "B",
            KeyCode::KeyC => "C",
            KeyCode::KeyD => "D",
            KeyCode::KeyE => "E",
            KeyCode::KeyF => "F",
            KeyCode::KeyG => "G",
            KeyCode::KeyH => "H",
            KeyCode::KeyI => "I",
            KeyCode::KeyJ => "J",
            KeyCode::KeyK => "K",
            KeyCode::KeyL => "L",
            KeyCode::KeyM => "M",
            KeyCode::KeyN => "N",
            KeyCode::KeyO => "O",
            KeyCode::KeyP => "P",
            KeyCode::KeyQ => "Q",
            KeyCode::KeyR => "R",
            KeyCode::KeyS => "S",
            KeyCode::KeyT => "T",
            KeyCode::KeyU => "U",
            KeyCode::KeyV => "V",
            KeyCode::KeyW => "W",
            KeyCode::KeyX => "X",
            KeyCode::KeyY => "Y",
            KeyCode::KeyZ => "Z",
            KeyCode::Digit0 => "0",
            KeyCode::Digit1 => "1",
            KeyCode::Digit2 => "2",
            KeyCode::Digit3 => "3",
            KeyCode::Digit4 => "4",
            KeyCode::Digit5 => "5",
            KeyCode::Digit6 => "6",
            KeyCode::Digit7 => "7",
            KeyCode::Digit8 => "8",
            KeyCode::Digit9 => "9",
            KeyCode::Space => "Space",
            KeyCode::Tab => "Tab",
            KeyCode::CapsLock => "CapsLock",
            KeyCode::Escape => "Escape",
            KeyCode::Backquote => "`",
        }
    }
}

/// 快捷键绑定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyBinding {
    /// 主键 (必须) - 触发录音的主要按键
    #[serde(default)]
    pub key: KeyCode,

    /// 修饰键 (可选) - 需要同时按住的修饰键
    #[serde(default)]
    pub modifiers: Vec<KeyCode>,
}

impl Default for HotkeyBinding {
    fn default() -> Self {
        Self {
            key: KeyCode::Alt,
            modifiers: vec![],
        }
    }
}

impl HotkeyBinding {
    /// 获取显示文本
    pub fn display_text(&self) -> String {
        let mut parts: Vec<&str> = self.modifiers.iter().map(|k| k.display_name()).collect();
        parts.push(self.key.display_name());
        parts.join(" + ")
    }
}

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
    /// 新的快捷键绑定
    #[serde(default)]
    pub binding: HotkeyBinding,

    /// 兼容旧配置: 旧的 trigger_key 字段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_key: Option<String>,

    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            binding: HotkeyBinding::default(),
            trigger_key: None,
            enabled: true,
        }
    }
}

impl HotkeyConfig {
    /// 从旧配置迁移
    pub fn migrate(&mut self) {
        if let Some(ref old_key) = self.trigger_key {
            // 旧配置存在，执行迁移
            self.binding = match old_key.as_str() {
                "Alt" => HotkeyBinding {
                    key: KeyCode::Alt,
                    modifiers: vec![],
                },
                "Control" => HotkeyBinding {
                    key: KeyCode::Control,
                    modifiers: vec![],
                },
                _ => HotkeyBinding::default(),
            };
            self.trigger_key = None;
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
