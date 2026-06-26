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

<div class="rounded-3xl border border-[#8dddd5]/20 bg-[#8dddd5]/10 p-5 shadow-lg shadow-black/20">
  <div class="mb-5 flex items-center gap-3">
    <div class="flex h-11 w-11 items-center justify-center rounded-2xl bg-[#8dddd5]/15 text-[#bff5ef]">
      <Link class="h-5 w-5" />
    </div>
    <div>
      <h3 class="font-bold text-[#effffb]">分享連結已生成</h3>
      <p class="text-sm text-[#bdebe5]">複製連結，或直接開啟檢查下載頁。</p>
    </div>
  </div>

  <div class="space-y-4">
    <div>
      <label for="download-url" class="text-sm font-semibold text-[#effffb]">
        分享連結
      </label>
      <div class="mt-2 grid gap-2 sm:grid-cols-[1fr_auto]">
        <input
          id="download-url"
          type="url"
          value={url}
          readonly
          class="min-h-11 w-full min-w-0 rounded-2xl border border-[#f7efe6]/10 bg-[#07111f]/70 px-4 py-3 text-sm text-[#f7efe6] shadow-inner shadow-black/20 outline-none focus-visible:border-[#8dddd5] focus-visible:ring-2 focus-visible:ring-[#8dddd5]/40"
        />
        <button
          type="button"
          onclick={handleCopy}
          class="inline-flex min-h-11 shrink-0 cursor-pointer items-center justify-center gap-2 whitespace-nowrap rounded-2xl px-4 py-3 text-sm font-black transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#8dddd5] {copied ? 'bg-[#7ee2b8] text-[#07111f]' : 'bg-[#fff8ee] text-[#07111f] hover:bg-[#bff5ef]'}"
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
      class="inline-flex min-h-11 w-full cursor-pointer items-center justify-center gap-2 whitespace-nowrap rounded-2xl border border-[#f7efe6]/10 bg-[#f7efe6]/[0.07] px-4 py-3 text-sm font-bold text-[#fff8ee] transition hover:bg-[#f7efe6]/[0.12] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#8dddd5]"
    >
      <ExternalLink class="h-4 w-4" />
      打開連結
    </a>
  </div>

  <div class="mt-5 border-t border-white/10 pt-4">
    <p class="text-xs font-medium uppercase tracking-[0.2em] text-[#ffd5c5]/70">sealed for 7 days</p>
  </div>
</div>
