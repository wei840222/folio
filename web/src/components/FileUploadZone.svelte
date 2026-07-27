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
  <div class="rounded-xl border border-primary-border bg-primary-soft p-8 text-center" aria-live="polite" aria-busy="true">
    <div class="mx-auto mb-4 h-10 w-10 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
    <h3 class="text-lg font-bold text-text">上傳中...</h3>
    <p class="mt-2 text-sm text-text-secondary">正在處理檔案，請稍等一下。</p>
  </div>
{:else}
  <button
    type="button"
    class="group relative w-full cursor-pointer overflow-hidden rounded-upload border-2 border-dashed p-8 text-center transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus sm:p-10 {isDragOver ? 'scale-[1.01] border-primary bg-surface shadow-sm' : 'border-border bg-surface hover:border-primary-focus hover:bg-primary-soft'}"
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={openFilePicker}
    aria-label="上傳檔案區域，點擊或拖曳檔案到此處"
  >
    <div class="pointer-events-none absolute inset-x-8 top-0 h-1 rounded-b-full bg-primary"></div>
    <div class="space-y-5">
      <div class="mx-auto flex h-20 w-20 items-center justify-center rounded-xl border border-primary-border bg-primary-soft text-primary transition group-hover:scale-105">
        <Upload class="h-9 w-9" />
      </div>

      <div>
        <h3 class="text-xl font-bold text-text">
          拖曳檔案到這裡
        </h3>
        <p class="mx-auto mt-2 max-w-sm text-sm leading-6 text-text-secondary">
          或點擊選擇檔案。支援所有類型，上限 25MB。
        </p>
      </div>

      <span class="inline-flex min-h-11 items-center justify-center gap-2 rounded-lg bg-primary px-5 py-3 text-sm font-extrabold text-on-primary transition group-hover:bg-primary-hover">
        <FileText class="h-4 w-4" />
        拖曳上傳
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
