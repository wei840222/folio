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

<div class="bg-blue-50 border border-blue-200 rounded-lg p-6">
  <div class="flex items-center gap-3 mb-4">
    <div class="w-10 h-10 bg-blue-100 rounded-full flex items-center justify-center">
      <Link class="w-5 h-5 text-blue-600" />
    </div>
    <div>
      <h3 class="font-semibold text-blue-800">分享連結已生成</h3>
      <p class="text-sm text-blue-600">您可以透過連結分享檔案。</p>
    </div>
  </div>

  <div class="space-y-3">
    <div>
      <label for="download-url" class="text-sm font-medium text-gray-700">
        分享連結
      </label>
      <div class="flex gap-2 mt-1">
        <input
          id="download-url"
          type="url"
          value={url}
          readonly
          class="flex h-9 w-full min-w-0 rounded-md border border-gray-300 bg-white px-3 py-1 text-base shadow-xs outline-none focus-visible:border-blue-500 focus-visible:ring-2 focus-visible:ring-blue-500/50"
        />
        <button
          type="button"
          onclick={handleCopy}
          class="inline-flex shrink-0 items-center justify-center gap-1 whitespace-nowrap rounded-md border border-gray-300 bg-white px-3 py-1.5 text-sm font-medium shadow-xs transition-all hover:bg-gray-50 cursor-pointer {copied ? 'bg-gray-100' : ''}"
        >
          <Copy class="w-4 h-4" />
          {copied ? '已複製' : '複製'}
        </button>
      </div>
    </div>

    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition-all hover:bg-blue-700 w-full cursor-pointer"
    >
      <ExternalLink class="w-4 h-4" />
      打開連結
    </a>
  </div>

  <div class="mt-4 pt-4 border-t border-blue-200">
    <p class="text-xs text-blue-600">連結有效時間：7天</p>
  </div>
</div>
