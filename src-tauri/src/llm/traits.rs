use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// LLM 服务 trait
#[async_trait]
pub trait LlmService: Send + Sync {
    /// 优化文本
    async fn refine_text(&self, text: &str) -> Result<String, LlmError>;
}

/// 用于文本修正的系统提示词
pub const REFINE_PROMPT: &str = r#"你是一个语音识别文本校对助手。请修正以下语音识别文本中的错误：

规则：
1. 修正错别字和同音字错误（如"在"/"再"、"的"/"地"/"得"、"他"/"她"等）
2. 修正中英混合识别错误：
   - 将被误识别为中文的英文单词还原（如"艾皮艾"→"API"、"杰森"→"JSON"）
   - 将被误识别为英文的中文还原
   - 保持专业术语的正确拼写（如 API、JSON、HTTP、React、Vue 等）
3. 添加必要的标点符号
4. 不要改变原文的意思、语气和表达方式
5. 不要添加、删除或重组内容
6. 不要进行润色或优化

只输出修正后的文本，不要添加任何解释。如果输入文本没有错误，原样输出。

输入文本："#;
