<script setup lang="ts">
defineProps<{
  isRecording: boolean;
  isProcessing: boolean;
}>();
</script>

<template>
  <div v-if="isRecording || isProcessing" class="indicator-overlay">
    <div
      class="indicator"
      :class="{ recording: isRecording, processing: isProcessing }"
    >
      <template v-if="isRecording">
        <div class="pulse-ring"></div>
        <div class="mic-icon">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path
              d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"
            />
            <path
              d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"
            />
          </svg>
        </div>
        <span class="label">正在听写...</span>
      </template>
      <template v-else-if="isProcessing">
        <div class="spinner"></div>
        <span class="label">处理中...</span>
      </template>
    </div>
  </div>
</template>

<style scoped>
.indicator-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.3);
  z-index: 9999;
  pointer-events: none;
}

.indicator {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 2rem 3rem;
  background: rgba(0, 0, 0, 0.85);
  border-radius: 16px;
  color: white;
  position: relative;
}

.indicator.recording {
  background: rgba(239, 68, 68, 0.9);
}

.indicator.processing {
  background: rgba(59, 130, 246, 0.9);
}

.pulse-ring {
  position: absolute;
  width: 80px;
  height: 80px;
  border: 3px solid rgba(255, 255, 255, 0.5);
  border-radius: 50%;
  animation: pulse 1.5s ease-out infinite;
}

@keyframes pulse {
  0% {
    transform: scale(0.8);
    opacity: 1;
  }
  100% {
    transform: scale(1.5);
    opacity: 0;
  }
}

.mic-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 1rem;
  animation: mic-pulse 1s ease-in-out infinite;
}

@keyframes mic-pulse {
  0%,
  100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.1);
  }
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 1rem;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.label {
  font-size: 1rem;
  font-weight: 500;
  letter-spacing: 0.5px;
}
</style>
