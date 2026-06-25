<script lang="ts">
  import { Upload, FileText } from '@lucide/svelte';

  let {
    onfileselect,
    isUploading = false,
  }: {
    onfileselect: (file: File) => void;
    isUploading?: boolean;
  } = $props();

  let isDragOver = $state(false);
  let fileInput = $state<HTMLInputElement>();

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragOver = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;

    const files = Array.from(e.dataTransfer?.files ?? []);
    if (files.length > 0) {
      onfileselect(files[0]);
    }
  }

  function handleFileInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    if (files && files.length > 0) {
      onfileselect(files[0]);
    }
  }

  function openFilePicker() {
    fileInput?.click();
  }
</script>

{#if isUploading}
  <div class="border-2 border-dashed border-blue-300 rounded-lg p-12 text-center bg-blue-50">
    <div class="animate-spin w-8 h-8 border-3 border-blue-600 border-t-transparent rounded-full mx-auto mb-4"></div>
    <h3 class="text-lg font-semibold text-blue-800 mb-2">上傳中...</h3>
    <p class="text-blue-600">請稍後，檔案處理中...</p>
  </div>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed rounded-lg p-12 text-center transition-all duration-200 cursor-pointer {isDragOver ? 'border-blue-500 bg-blue-50 scale-105' : 'border-gray-300 hover:border-blue-400 hover:bg-gray-50'}"
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={openFilePicker}
    role="button"
    tabindex="0"
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openFilePicker(); } }}
    aria-label="上傳檔案區域，點擊或拖曳檔案到此處"
  >
    <div class="space-y-4">
      <div class="mx-auto w-16 h-16 bg-blue-100 rounded-full flex items-center justify-center">
        <Upload class="w-8 h-8 text-blue-600" />
      </div>

      <div>
        <h3 class="text-lg font-semibold text-gray-900 mb-2">
          拖曳檔案到此處或點選上傳按鈕
        </h3>
        <p class="text-gray-600 mb-4">支援所有檔案類型，最大大小5MB</p>
      </div>

      <button
        type="button"
        class="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition-all hover:bg-blue-700 cursor-pointer"
        onclick={(e) => { e.stopPropagation(); openFilePicker(); }}
      >
        <FileText class="w-4 h-4" />
        選擇檔案
      </button>
    </div>

    <input
      bind:this={fileInput}
      type="file"
      class="hidden"
      onchange={handleFileInput}
      aria-hidden="true"
    />
  </div>
{/if}
