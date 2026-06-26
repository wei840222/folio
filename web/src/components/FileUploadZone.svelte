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
  <div class="rounded-3xl border border-[#8dddd5]/30 bg-[#8dddd5]/10 p-8 text-center" aria-live="polite" aria-busy="true">
    <div class="mx-auto mb-4 h-10 w-10 animate-spin rounded-full border-2 border-[#8dddd5] border-t-transparent"></div>
    <h3 class="text-lg font-bold text-[#effffb]">封存中...</h3>
    <p class="mt-2 text-sm text-[#bdebe5]">正在處理檔案，請稍等一下。</p>
  </div>
{:else}
  <button
    type="button"
    class="group relative w-full cursor-pointer overflow-hidden rounded-3xl border border-dashed p-8 text-center transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#8dddd5] sm:p-10 {isDragOver ? 'scale-[1.01] border-[#8dddd5] bg-[#8dddd5]/15 shadow-2xl shadow-black/40' : 'border-[#f7efe6]/20 bg-[#f7efe6]/[0.04] hover:border-[#8dddd5]/70 hover:bg-[#8dddd5]/10'}"
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={openFilePicker}
    aria-label="上傳檔案區域，點擊或拖曳檔案到此處"
  >
    <div class="pointer-events-none absolute inset-x-8 top-0 h-1 rounded-b-full bg-gradient-to-r from-transparent via-[#d96c4a]/80 to-transparent"></div>
    <div class="space-y-5">
      <div class="mx-auto flex h-20 w-20 items-center justify-center rounded-[1.75rem] border border-[#8dddd5]/20 bg-[#8dddd5]/10 text-[#bff5ef] shadow-lg shadow-black/30 transition group-hover:scale-105">
        <Upload class="h-9 w-9" />
      </div>

      <div>
        <h3 class="text-xl font-black text-[#fff8ee]">
          把檔案投入封存口
        </h3>
        <p class="mx-auto mt-2 max-w-sm text-sm leading-6 text-[#d8cfc3]">
          或點一下開啟檔案選擇器。支援所有檔案類型，檔案上限 25MB。
        </p>
      </div>

      <span class="inline-flex min-h-11 items-center justify-center gap-2 rounded-2xl bg-[#8dddd5] px-5 py-3 text-sm font-black text-[#07111f] shadow-lg shadow-black/20 transition group-hover:bg-[#fff8ee]">
        <FileText class="h-4 w-4" />
        Drop a file
      </span>
    </div>

    <input
      bind:this={fileInput}
      type="file"
      class="hidden"
      onchange={handleFileInput}
      aria-hidden="true"
    />
  </button>
{/if}
