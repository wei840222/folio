<script lang="ts">
  import { Upload, Cpu, Sparkles, RefreshCw } from '@lucide/svelte';

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
  <div class="relative overflow-hidden rounded-2xl cyber-glass p-10 text-center space-y-4 border border-cyber-cyan/50 shadow-2xl" aria-live="polite" aria-busy="true">
    <div class="laser-line"></div>
    <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-cyber-cyan/10 border border-cyber-cyan/40 text-cyber-cyan">
      <RefreshCw class="h-8 w-8 animate-spin text-cyber-cyan" />
    </div>
    <div>
      <h3 class="font-mono text-base font-bold text-text uppercase tracking-wider">FOLIO UPLINK IN PROGRESS...</h3>
      <p class="mt-1 font-mono text-xs text-text-secondary">正在寫入 Folio 成果典藏庫，請稍候。</p>
    </div>
    <div class="w-full bg-background/80 rounded-full h-1.5 overflow-hidden border border-cyber-cyan/20">
      <div class="bg-gradient-to-r from-cyber-cyan via-cyber-purple to-cyber-emerald h-full animate-pulse w-full"></div>
    </div>
  </div>
{:else}
  <button
    type="button"
    class="group relative w-full cursor-pointer overflow-hidden rounded-2xl border-2 border-dashed p-8 text-center transition-all duration-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyber-cyan sm:p-10 {isDragOver ? 'scale-[1.02] border-cyber-cyan bg-cyber-cyan/10 shadow-[0_0_30px_rgba(0,242,254,0.3)]' : 'border-border/60 cyber-glass hover:border-cyber-cyan/60 hover:shadow-[0_0_20px_rgba(0,242,254,0.15)]'}"
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={openFilePicker}
    aria-label="Folio 檔案典藏上傳區域，點擊或拖曳檔案到此處"
  >
    <!-- Laser line effect when hovered or dragover -->
    <div class="laser-line opacity-0 group-hover:opacity-100 transition-opacity"></div>

    <div class="pointer-events-none absolute inset-x-8 top-0 h-[2px] bg-gradient-to-r from-transparent via-cyber-cyan to-transparent"></div>
    
    <div class="space-y-5 relative z-10">
      <div class="mx-auto flex h-20 w-20 items-center justify-center rounded-2xl border border-cyber-cyan/30 bg-cyber-cyan/10 text-cyber-cyan transition-all duration-300 group-hover:scale-110 group-hover:border-cyber-cyan group-hover:bg-cyber-cyan/20 group-hover:shadow-[0_0_15px_rgba(0,242,254,0.4)]">
        <Upload class="h-9 w-9 text-cyber-cyan" />
      </div>

      <div>
        <div class="inline-flex items-center gap-1.5 mb-2 px-2.5 py-0.5 rounded-full bg-cyber-cyan/10 border border-cyber-cyan/30 text-[10px] font-mono text-cyber-cyan uppercase">
          <Cpu class="h-3 w-3" />
          FOLIO ARTIFACT UPLINK
        </div>
        <h3 class="font-mono text-lg font-bold text-text group-hover:text-cyber-cyan transition">
          拖曳檔案至此區域上傳
        </h3>
        <p class="mx-auto mt-1.5 max-w-sm text-xs leading-5 text-text-secondary font-mono">
          支援所有類型（Markdown 報告、Code、圖片、文件等），上限 25MB。
        </p>
      </div>

      <div class="flex items-center justify-center gap-2">
        <span class="inline-flex min-h-11 items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-cyber-cyan via-teal-400 to-cyber-emerald px-6 py-2.5 font-mono text-xs font-extrabold text-on-primary transition-all duration-200 group-hover:shadow-[0_0_20px_rgba(0,242,254,0.5)] group-hover:scale-105">
          <Sparkles class="h-4 w-4" />
          選擇 / 拖曳檔案
        </span>
      </div>
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

