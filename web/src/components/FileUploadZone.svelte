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
  <div class="rounded-3xl border border-cyan-300/30 bg-cyan-300/10 p-8 text-center" aria-live="polite" aria-busy="true">
    <div class="mx-auto mb-4 h-10 w-10 animate-spin rounded-full border-2 border-cyan-200 border-t-transparent"></div>
    <h3 class="text-lg font-bold text-cyan-50">上傳中...</h3>
    <p class="mt-2 text-sm text-cyan-100/70">正在處理檔案，請稍等一下。</p>
  </div>
{:else}
  <button
    type="button"
    class="group relative w-full overflow-hidden rounded-3xl border border-dashed p-8 text-center transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300 sm:p-10 cursor-pointer {isDragOver ? 'scale-[1.01] border-cyan-200 bg-cyan-300/15 shadow-2xl shadow-cyan-950/40' : 'border-white/20 bg-white/[0.04] hover:border-cyan-200/70 hover:bg-cyan-300/10'}"
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={openFilePicker}
    aria-label="上傳檔案區域，點擊或拖曳檔案到此處"
  >
    <div class="pointer-events-none absolute inset-x-8 top-0 h-px bg-gradient-to-r from-transparent via-cyan-200/70 to-transparent"></div>
    <div class="space-y-5">
      <div class="mx-auto flex h-20 w-20 items-center justify-center rounded-[1.75rem] border border-cyan-200/20 bg-cyan-200/10 text-cyan-100 shadow-lg shadow-cyan-950/30 transition group-hover:scale-105">
        <Upload class="h-9 w-9" />
      </div>

      <div>
        <h3 class="text-xl font-black text-white">
          拖曳檔案到這裡
        </h3>
        <p class="mx-auto mt-2 max-w-sm text-sm leading-6 text-slate-300">
          或點一下開啟檔案選擇器。支援所有檔案類型，最大大小 5MB。
        </p>
      </div>

      <span class="inline-flex min-h-11 items-center justify-center gap-2 rounded-2xl bg-cyan-200 px-5 py-3 text-sm font-black text-slate-950 shadow-lg shadow-cyan-950/20 transition group-hover:bg-white">
        <FileText class="h-4 w-4" />
        選擇檔案
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
