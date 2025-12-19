<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

type TabType = 'asr' | 'llm' | 'hotkey';
const activeTab = ref<TabType>('asr');

// ASR 配置
const asrProvider = ref('DashScope');
const dashscopeApiKey = ref('');
const dashscopeModel = ref('paraformer-realtime-v2');
const openaiAsrApiKey = ref('');
const openaiAsrModel = ref('whisper-1');
const openaiAsrLanguage = ref('zh');
const funasrEndpoint = ref('http://localhost:10095');

// LLM 配置
const llmEnabled = ref(true);
const llmProvider = ref('DashScope');
const llmApiKey = ref('');
const llmModel = ref('qwen-plus');
const ollamaEndpoint = ref('http://localhost:11434');
const ollamaModel = ref('qwen3:8b');

// 快捷键配置
const triggerKey = ref('Alt');

// 测试状态
const testingDashscope = ref(false);
const testingOpenai = ref(false);
const testingFunasr = ref(false);
const testingOllama = ref(false);
const testResult = ref<{ success: boolean; message: string } | null>(null);

// 保存状态
const saving = ref(false);
const saveMessage = ref<{ success: boolean; message: string } | null>(null);

async function testDashscopeApi() {
  if (!dashscopeApiKey.value) {
    testResult.value = { success: false, message: 'API Key 不能为空' };
    return;
  }
  testingDashscope.value = true;
  testResult.value = null;
  try {
    const result = await invoke<string>('test_dashscope_api', { apiKey: dashscopeApiKey.value });
    testResult.value = { success: true, message: result };
  } catch (e) {
    testResult.value = { success: false, message: e as string };
  } finally {
    testingDashscope.value = false;
  }
}

async function testOpenaiAsrApi() {
  if (!openaiAsrApiKey.value) {
    testResult.value = { success: false, message: 'API Key 不能为空' };
    return;
  }
  testingOpenai.value = true;
  testResult.value = null;
  try {
    const result = await invoke<string>('test_openai_api', { apiKey: openaiAsrApiKey.value });
    testResult.value = { success: true, message: result };
  } catch (e) {
    testResult.value = { success: false, message: e as string };
  } finally {
    testingOpenai.value = false;
  }
}

async function testFunasrApi() {
  if (!funasrEndpoint.value) {
    testResult.value = { success: false, message: '服务地址不能为空' };
    return;
  }
  testingFunasr.value = true;
  testResult.value = null;
  try {
    const result = await invoke<string>('test_funasr_api', { endpoint: funasrEndpoint.value });
    testResult.value = { success: true, message: result };
  } catch (e) {
    testResult.value = { success: false, message: e as string };
  } finally {
    testingFunasr.value = false;
  }
}

async function testOllamaApi() {
  if (!ollamaEndpoint.value) {
    testResult.value = { success: false, message: '服务地址不能为空' };
    return;
  }
  if (!ollamaModel.value) {
    testResult.value = { success: false, message: '模型名称不能为空' };
    return;
  }
  testingOllama.value = true;
  testResult.value = null;
  try {
    const result = await invoke<string>('test_ollama_api', { endpoint: ollamaEndpoint.value, model: ollamaModel.value });
    testResult.value = { success: true, message: result };
  } catch (e) {
    testResult.value = { success: false, message: e as string };
  } finally {
    testingOllama.value = false;
  }
}

async function loadConfig() {
  try {
    const config = await invoke<any>('get_config');
    if (config) {
      // 加载 ASR 配置
      asrProvider.value = config.asr?.provider || 'DashScope';
      dashscopeApiKey.value = config.asr?.dashscope?.api_key || '';
      dashscopeModel.value = config.asr?.dashscope?.model || 'paraformer-realtime-v2';
      openaiAsrApiKey.value = config.asr?.openai?.api_key || '';
      openaiAsrModel.value = config.asr?.openai?.model || 'whisper-1';
      openaiAsrLanguage.value = config.asr?.openai?.language || 'zh';
      funasrEndpoint.value = config.asr?.funasr?.endpoint || 'http://localhost:10095';

      // 加载 LLM 配置
      llmEnabled.value = config.llm?.enabled ?? true;
      llmProvider.value = config.llm?.provider || 'DashScope';
      if (llmProvider.value === 'DashScope') {
        llmApiKey.value = config.llm?.dashscope?.api_key || config.asr?.dashscope?.api_key || '';
        llmModel.value = config.llm?.dashscope?.model || 'qwen-plus';
      } else if (llmProvider.value === 'Ollama') {
        ollamaEndpoint.value = config.llm?.ollama?.endpoint || 'http://localhost:11434';
        ollamaModel.value = config.llm?.ollama?.model || 'qwen3:8b';
      } else {
        llmApiKey.value = config.llm?.openai?.api_key || '';
        llmModel.value = config.llm?.openai?.model || 'gpt-4o-mini';
      }

      // 加载快捷键配置
      triggerKey.value = config.hotkey?.trigger_key || 'Alt';
    }
  } catch (e) {
    console.error('Failed to load config:', e);
  }
}

async function saveConfig() {
  saving.value = true;
  saveMessage.value = null;
  try {
    const config: any = {
      hotkey: { trigger_key: triggerKey.value, enabled: true },
      asr: {
        provider: asrProvider.value,
      },
      llm: {
        enabled: llmEnabled.value,
        provider: llmProvider.value,
      },
      output: {
        restore_clipboard: true,
        paste_delay_ms: 50,
      },
    };

    // ASR 配置
    if (asrProvider.value === 'DashScope') {
      config.asr.dashscope = {
        api_key: dashscopeApiKey.value,
        model: dashscopeModel.value,
      };
    } else if (asrProvider.value === 'OpenAIWhisper') {
      config.asr.openai = {
        api_key: openaiAsrApiKey.value,
        model: openaiAsrModel.value,
        language: openaiAsrLanguage.value,
      };
    } else if (asrProvider.value === 'FunAsr') {
      config.asr.funasr = {
        endpoint: funasrEndpoint.value,
      };
    }

    // LLM 配置
    if (llmEnabled.value) {
      if (llmProvider.value === 'DashScope') {
        config.llm.dashscope = {
          api_key: llmApiKey.value,
          model: llmModel.value,
        };
      } else if (llmProvider.value === 'OpenAI') {
        config.llm.openai = {
          api_key: llmApiKey.value,
          model: llmModel.value,
          temperature: 0.3,
          max_tokens: 2000,
        };
      } else if (llmProvider.value === 'Ollama') {
        config.llm.ollama = {
          endpoint: ollamaEndpoint.value,
          model: ollamaModel.value,
        };
      }
    }

    await invoke('save_config', { config });
    saveMessage.value = { success: true, message: '保存成功' };
    setTimeout(() => {
      saveMessage.value = null;
    }, 2000);
  } catch (e) {
    console.error('Failed to save config:', e);
    saveMessage.value = { success: false, message: '保存失败: ' + e };
  } finally {
    saving.value = false;
  }
}

onMounted(() => {
  loadConfig();
});
</script>

<template>
  <div class="settings">
    <div class="sidebar">
      <h1 class="app-title">Vhisper</h1>
      <nav class="nav">
        <button :class="{ active: activeTab === 'asr' }" @click="activeTab = 'asr'">
          <span class="nav-icon">🎤</span>
          语音识别
        </button>
        <button :class="{ active: activeTab === 'llm' }" @click="activeTab = 'llm'">
          <span class="nav-icon">✨</span>
          文本优化
        </button>
        <button :class="{ active: activeTab === 'hotkey' }" @click="activeTab = 'hotkey'">
          <span class="nav-icon">⌨️</span>
          快捷键
        </button>
      </nav>
    </div>

    <div class="main">
      <div class="content">
        <!-- ASR Tab -->
        <template v-if="activeTab === 'asr'">
          <h2>语音识别设置</h2>
          <div class="form-group">
            <label for="asr-provider">ASR 服务商</label>
            <select id="asr-provider" v-model="asrProvider">
              <option value="DashScope">阿里云 DashScope</option>
              <option value="OpenAIWhisper">OpenAI Whisper</option>
              <option value="FunAsr">FunASR (本地)</option>
            </select>
          </div>

          <!-- DashScope -->
          <template v-if="asrProvider === 'DashScope'">
            <div class="form-group">
              <label for="dashscope-api-key">API Key</label>
              <div class="input-with-button">
                <input
                  type="password"
                  id="dashscope-api-key"
                  v-model="dashscopeApiKey"
                  placeholder="sk-..."
                />
                <button
                  class="btn-test"
                  @click="testDashscopeApi"
                  :disabled="testingDashscope"
                >
                  {{ testingDashscope ? '测试中...' : '测试' }}
                </button>
              </div>
              <p class="hint">从阿里云百炼控制台获取 API Key</p>
              <p
                v-if="testResult && asrProvider === 'DashScope'"
                class="test-result"
                :class="{ success: testResult.success, error: !testResult.success }"
              >
                {{ testResult.message }}
              </p>
            </div>
            <div class="form-group">
              <label for="dashscope-model">模型</label>
              <select id="dashscope-model" v-model="dashscopeModel">
                <option value="paraformer-realtime-v2">paraformer-realtime-v2 (推荐)</option>
                <option value="paraformer-realtime-v1">paraformer-realtime-v1</option>
                <option value="paraformer-realtime-8k-v2">paraformer-realtime-8k-v2</option>
              </select>
            </div>
          </template>

          <!-- OpenAI Whisper -->
          <template v-else-if="asrProvider === 'OpenAIWhisper'">
            <div class="form-group">
              <label for="openai-asr-api-key">API Key</label>
              <div class="input-with-button">
                <input
                  type="password"
                  id="openai-asr-api-key"
                  v-model="openaiAsrApiKey"
                  placeholder="sk-..."
                />
                <button
                  class="btn-test"
                  @click="testOpenaiAsrApi"
                  :disabled="testingOpenai"
                >
                  {{ testingOpenai ? '测试中...' : '测试' }}
                </button>
              </div>
              <p
                v-if="testResult && asrProvider === 'OpenAIWhisper'"
                class="test-result"
                :class="{ success: testResult.success, error: !testResult.success }"
              >
                {{ testResult.message }}
              </p>
            </div>
            <div class="form-group">
              <label for="openai-asr-model">模型</label>
              <select id="openai-asr-model" v-model="openaiAsrModel">
                <option value="whisper-1">whisper-1</option>
              </select>
            </div>
            <div class="form-group">
              <label for="openai-asr-language">语言</label>
              <select id="openai-asr-language" v-model="openaiAsrLanguage">
                <option value="zh">中文</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </select>
            </div>
          </template>

          <!-- FunASR -->
          <template v-else-if="asrProvider === 'FunAsr'">
            <div class="form-group">
              <label for="funasr-endpoint">服务地址</label>
              <div class="input-with-button">
                <input
                  type="text"
                  id="funasr-endpoint"
                  v-model="funasrEndpoint"
                  placeholder="http://localhost:10095"
                />
                <button
                  class="btn-test"
                  @click="testFunasrApi"
                  :disabled="testingFunasr"
                >
                  {{ testingFunasr ? '测试中...' : '测试' }}
                </button>
              </div>
              <p class="hint">本地 FunASR 服务的 HTTP API 地址</p>
              <p
                v-if="testResult && asrProvider === 'FunAsr'"
                class="test-result"
                :class="{ success: testResult.success, error: !testResult.success }"
              >
                {{ testResult.message }}
              </p>
            </div>
          </template>
        </template>

        <!-- LLM Tab -->
        <template v-else-if="activeTab === 'llm'">
          <h2>文本优化设置</h2>
          <div class="form-group">
            <label class="checkbox">
              <input type="checkbox" v-model="llmEnabled" />
              启用 LLM 文本优化
            </label>
            <p class="hint">对语音识别结果进行优化，修正错误、添加标点</p>
          </div>

          <template v-if="llmEnabled">
            <div class="form-group">
              <label for="llm-provider">LLM 服务商</label>
              <select id="llm-provider" v-model="llmProvider">
                <option value="DashScope">阿里云通义千问 (复用 ASR API Key)</option>
                <option value="OpenAI">OpenAI</option>
                <option value="Ollama">Ollama (本地)</option>
              </select>
            </div>

            <!-- DashScope LLM -->
            <template v-if="llmProvider === 'DashScope'">
              <div class="form-group">
                <label for="llm-api-key">API Key</label>
                <input
                  type="password"
                  id="llm-api-key"
                  v-model="llmApiKey"
                  placeholder="留空则复用语音识别的 API Key"
                />
                <p class="hint">可以留空，将自动使用语音识别的 API Key</p>
              </div>

              <div class="form-group">
                <label for="llm-model">模型</label>
                <select id="llm-model" v-model="llmModel">
                  <option value="qwen-plus">qwen-plus (推荐)</option>
                  <option value="qwen-max">qwen-max (强大)</option>
                  <option value="qwen-long">qwen-long (长文本)</option>
                </select>
              </div>
            </template>

            <!-- OpenAI LLM -->
            <template v-else-if="llmProvider === 'OpenAI'">
              <div class="form-group">
                <label for="llm-api-key">API Key</label>
                <input
                  type="password"
                  id="llm-api-key"
                  v-model="llmApiKey"
                  placeholder="sk-..."
                />
              </div>

              <div class="form-group">
                <label for="llm-model">模型</label>
                <input
                  type="text"
                  id="llm-model"
                  v-model="llmModel"
                  placeholder="gpt-4o-mini"
                />
              </div>
            </template>

            <!-- Ollama LLM -->
            <template v-else-if="llmProvider === 'Ollama'">
              <div class="form-group">
                <label for="ollama-endpoint">服务地址</label>
                <input
                  type="text"
                  id="ollama-endpoint"
                  v-model="ollamaEndpoint"
                  placeholder="http://localhost:11434"
                />
                <p class="hint">本地 Ollama 服务地址</p>
              </div>

              <div class="form-group">
                <label for="ollama-model">模型</label>
                <div class="input-with-button">
                  <input
                    type="text"
                    id="ollama-model"
                    v-model="ollamaModel"
                    placeholder="qwen3:8b"
                  />
                  <button
                    class="btn-test"
                    @click="testOllamaApi"
                    :disabled="testingOllama"
                  >
                    {{ testingOllama ? '测试中...' : '测试' }}
                  </button>
                </div>
                <p class="hint">已安装的 Ollama 模型名称</p>
                <p
                  v-if="testResult && llmProvider === 'Ollama'"
                  class="test-result"
                  :class="{ success: testResult.success, error: !testResult.success }"
                >
                  {{ testResult.message }}
                </p>
              </div>
            </template>
          </template>
        </template>

        <!-- Hotkey Tab -->
        <template v-else-if="activeTab === 'hotkey'">
          <h2>快捷键设置</h2>
          <div class="form-group">
            <label for="trigger-key">触发键</label>
            <select id="trigger-key" v-model="triggerKey">
              <option value="Alt">Option / Alt</option>
              <option value="Control">Control</option>
            </select>
          </div>
          <p class="hint">按住此键开始录音，松开后进行语音识别并输出文字</p>
        </template>
      </div>

      <div class="footer">
        <p
          v-if="saveMessage"
          class="save-message"
          :class="{ success: saveMessage.success, error: !saveMessage.success }"
        >
          {{ saveMessage.message }}
        </p>
        <button class="btn-primary" @click="saveConfig" :disabled="saving">
          {{ saving ? '保存中...' : '保存设置' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  height: 100vh;
  background: var(--bg-color, #f5f5f5);
}

.sidebar {
  width: 180px;
  background: var(--sidebar-bg, #fff);
  border-right: 1px solid var(--border-color, #e0e0e0);
  display: flex;
  flex-direction: column;
  padding: 1rem 0;
}

.app-title {
  font-size: 1.2rem;
  font-weight: 600;
  padding: 0.5rem 1rem 1rem;
  color: var(--text-color, #333);
  border-bottom: 1px solid var(--border-color, #e0e0e0);
  margin-bottom: 0.5rem;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0.5rem;
}

.nav button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  background: none;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  color: var(--text-secondary, #666);
  font-size: 0.9rem;
  text-align: left;
  transition: all 0.2s;
}

.nav button:hover {
  background: var(--hover-bg, #f0f0f0);
}

.nav button.active {
  background: var(--active-bg, #007aff);
  color: #fff;
}

.nav-icon {
  font-size: 1rem;
}

.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content {
  flex: 1;
  padding: 1.5rem 2rem;
  overflow-y: auto;
}

h2 {
  font-size: 1.3rem;
  font-weight: 600;
  margin-bottom: 1.5rem;
  color: var(--text-color, #333);
}

.form-group {
  margin-bottom: 1.25rem;
}

label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
  color: var(--text-color, #333);
}

label.checkbox {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
}

input[type='text'],
input[type='password'],
select {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--input-border, #ddd);
  border-radius: 8px;
  font-size: 1rem;
  box-sizing: border-box;
  background: var(--input-bg, #fff);
  color: var(--text-color, #333);
}

.input-with-button {
  display: flex;
  gap: 0.5rem;
}

.input-with-button input {
  flex: 1;
}

.btn-test {
  padding: 0.75rem 1rem;
  background: var(--btn-secondary-bg, #f0f0f0);
  border: 1px solid var(--input-border, #ddd);
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.9rem;
  white-space: nowrap;
  color: var(--text-color, #333);
}

.btn-test:hover:not(:disabled) {
  background: var(--btn-secondary-hover, #e0e0e0);
}

.btn-test:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.test-result,
.save-message {
  margin-top: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-radius: 6px;
  font-size: 0.85rem;
}

.test-result.success,
.save-message.success {
  background: #d4edda;
  color: #155724;
}

.test-result.error,
.save-message.error {
  background: #f8d7da;
  color: #721c24;
}

input:focus,
select:focus {
  outline: none;
  border-color: #007aff;
}

.hint {
  font-size: 0.85rem;
  color: var(--text-secondary, #888);
  margin-top: 0.5rem;
}

.footer {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 1rem;
  padding: 1rem 2rem;
  border-top: 1px solid var(--border-color, #e0e0e0);
  background: var(--sidebar-bg, #fff);
}

.btn-primary {
  padding: 0.6rem 1.5rem;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.9rem;
  background: #007aff;
  color: white;
  border: none;
}

.btn-primary:hover:not(:disabled) {
  background: #0066dd;
}

.btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@media (prefers-color-scheme: dark) {
  .settings {
    --bg-color: #1a1a1a;
    --sidebar-bg: #2a2a2a;
    --border-color: #444;
    --text-color: #eee;
    --text-secondary: #aaa;
    --hover-bg: #3a3a3a;
    --active-bg: #007aff;
    --input-bg: #333;
    --input-border: #555;
    --btn-secondary-bg: #444;
    --btn-secondary-hover: #555;
  }

  .test-result.success,
  .save-message.success {
    background: #1e4620;
    color: #a3d9a5;
  }

  .test-result.error,
  .save-message.error {
    background: #4a1c1c;
    color: #f5a5a5;
  }
}
</style>
