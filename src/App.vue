<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';
import Settings from './components/Settings.vue';
import RecordingIndicator from './components/RecordingIndicator.vue';

const isRecording = ref(false);
const isProcessing = ref(false);
const errorMessage = ref('');

onMounted(async () => {
  // 监听来自 Rust 的事件
  await listen('recording-started', () => {
    isRecording.value = true;
    isProcessing.value = false;
    errorMessage.value = '';
  });

  await listen('recording-stopped', () => {
    isRecording.value = false;
    isProcessing.value = true;
  });

  await listen('processing-complete', () => {
    isProcessing.value = false;
  });

  await listen<string>('processing-error', (event) => {
    isProcessing.value = false;
    errorMessage.value = event.payload;
    console.error('Processing error:', event.payload);
    // 5秒后清除错误信息
    setTimeout(() => {
      errorMessage.value = '';
    }, 5000);
  });
});
</script>

<template>
  <main>
    <Settings />

    <div v-if="errorMessage" class="error-toast">
      {{ errorMessage }}
    </div>

    <RecordingIndicator :is-recording="isRecording" :is-processing="isProcessing" />
  </main>
</template>

<style scoped>
main {
  min-height: 100vh;
  margin: 0;
  padding: 0;
}

.error-toast {
  position: fixed;
  bottom: 2rem;
  left: 50%;
  transform: translateX(-50%);
  background: #ef4444;
  color: white;
  padding: 0.75rem 1.5rem;
  border-radius: 8px;
  font-size: 0.9rem;
  max-width: 80%;
  text-align: center;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  z-index: 9999;
}
</style>
