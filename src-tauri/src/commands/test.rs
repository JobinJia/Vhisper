/// 测试通义千问 ASR API
#[tauri::command]
pub async fn test_qwen_api(api_key: String) -> Result<String, String> {
    vhisper_core::test_qwen_api(&api_key)
        .await
        .map_err(|e| e.to_string())
}

/// 测试 DashScope API
#[tauri::command]
pub async fn test_dashscope_api(api_key: String) -> Result<String, String> {
    vhisper_core::test_dashscope_api(&api_key)
        .await
        .map_err(|e| e.to_string())
}

/// 测试 OpenAI API
#[tauri::command]
pub async fn test_openai_api(api_key: String) -> Result<String, String> {
    vhisper_core::test_openai_api(&api_key)
        .await
        .map_err(|e| e.to_string())
}

/// 测试 FunASR API
#[tauri::command]
pub async fn test_funasr_api(endpoint: String) -> Result<String, String> {
    vhisper_core::test_funasr_api(&endpoint)
        .await
        .map_err(|e| e.to_string())
}

/// 测试 Ollama API
#[tauri::command]
pub async fn test_ollama_api(endpoint: String, model: String) -> Result<String, String> {
    vhisper_core::test_ollama_api(&endpoint, &model)
        .await
        .map_err(|e| e.to_string())
}
