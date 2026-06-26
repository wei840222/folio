<script lang="ts">
  import { Copy, Link, ExternalLink } from '@lucide/svelte';

  let { url }: { url: string } = $props();

  let copied = $state(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(url);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="rounded-3xl border border-cyan-300/20 bg-cyan-300/10 p-5 shadow-lg shadow-cyan-950/20">
  <div class="mb-5 flex items-center gap-3">
    <div class="flex h-11 w-11 items-center justify-center rounded-2xl bg-cyan-200/15 text-cyan-100">
      <Link class="h-5 w-5" />
    </div>
    <div>
      <h3 class="font-bold text-cyan-50">分享連結已生成</h3>
      <p class="text-sm text-cyan-100/70">複製連結，或直接開啟檢查下載頁。</p>
    </div>
  </div>

  <div class="space-y-4">
    <div>
      <label for="download-url" class="text-sm font-semibold text-cyan-50">
        分享連結
      </label>
      <div class="mt-2 grid gap-2 sm:grid-cols-[1fr_auto]">
        <input
          id="download-url"
          type="url"
          value={url}
          readonly
          class="min-h-11 w-full min-w-0 rounded-2xl border border-white/10 bg-slate-950/70 px-4 py-3 text-sm text-slate-100 shadow-inner shadow-black/20 outline-none focus-visible:border-cyan-300 focus-visible:ring-2 focus-visible:ring-cyan-300/40"
        />
        <button
          type="button"
          onclick={handleCopy}
          class="inline-flex min-h-11 shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-2xl px-4 py-3 text-sm font-black transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300 cursor-pointer {copied ? 'bg-emerald-300 text-slate-950' : 'bg-white text-slate-950 hover:bg-cyan-100'}"
          aria-live="polite"
        >
          <Copy class="h-4 w-4" />
          {copied ? '已複製' : '複製'}
        </button>
      </div>
    </div>

    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex min-h-11 w-full items-center justify-center gap-2 whitespace-nowrap rounded-2xl border border-white/10 bg-white/[0.07] px-4 py-3 text-sm font-bold text-white transition hover:bg-white/[0.12] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300 cursor-pointer"
    >
      <ExternalLink class="h-4 w-4" />
      打開連結
    </a>
  </div>

  <div class="mt-5 border-t border-white/10 pt-4">
    <p class="text-xs font-medium uppercase tracking-[0.2em] text-cyan-100/60">expires in 7 days</p>
  </div>
</div>
