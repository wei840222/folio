<script lang="ts">
  import { Copy, Check } from '@lucide/svelte';

  let { url }: { url: string } = $props();

  let copied = $state(false);

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(url);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (err) {
      console.error('複製失敗:', err);
    }
  }
</script>

<div class="space-y-3">
  <div class="rounded-2xl border border-blue-200/60 bg-white/80 p-4 backdrop-blur-sm">
    <p class="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-2">
      短連結
    </p>
    <div class="flex items-center gap-2">
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        class="flex-1 truncate rounded-xl border border-blue-200/60 bg-blue-50/40 px-4 py-3 font-mono text-sm font-bold text-blue-700 hover:bg-blue-50/70 hover:border-blue-300 transition"
      >
        {url}
      </a>
      <button
        type="button"
        onclick={copyToClipboard}
        class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl border border-slate-200/80 bg-white text-slate-600 transition hover:bg-blue-50 hover:text-blue-700 hover:border-blue-300"
        aria-label="複製到剪貼簿"
      >
        {#if copied}
          <Check class="h-5 w-5 text-emerald-600" />
        {:else}
          <Copy class="h-5 w-5" />
        {/if}
      </button>
    </div>
  </div>

  <p class="text-xs text-slate-500 text-center">
    預設保留 7 天，到期後自動刪除
  </p>
</div>
