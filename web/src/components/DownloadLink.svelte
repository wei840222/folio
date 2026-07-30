<script lang="ts">
  import { Copy, Check } from '@lucide/svelte';

  let { url, expiresAt }: { url: string; expiresAt: number | null } = $props();

  let copied = $state(false);
  let expirationLabel = $derived(
    expiresAt === null
      ? null
      : new Intl.DateTimeFormat('zh-TW', {
          dateStyle: 'medium',
          timeStyle: 'short',
        }).format(new Date(expiresAt * 1000)),
  );

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
  <div class="rounded-xl border border-border-subtle bg-surface p-4">
    <p class="folio-label mb-2 text-xs font-semibold uppercase text-text-muted">
      短連結
    </p>
    <div class="flex items-center gap-2">
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        class="flex-1 truncate rounded-md border border-primary-border bg-primary-soft px-4 py-3 font-mono text-sm font-bold text-primary-hover transition hover:border-primary-focus hover:bg-surface-subtle"
      >
        {url}
      </a>
      <button
        type="button"
        onclick={copyToClipboard}
        class="flex h-12 w-12 shrink-0 items-center justify-center rounded-md border border-border bg-surface text-text-secondary transition hover:border-primary-focus hover:bg-primary-soft hover:text-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus"
        aria-label="複製到剪貼簿"
      >
        {#if copied}
          <Check class="h-5 w-5 text-on-success-surface" />
        {:else}
          <Copy class="h-5 w-5" />
        {/if}
      </button>
    </div>
  </div>

  <p class="text-center text-xs text-text-muted">
    到期時間：{expirationLabel ?? '無法顯示'}
  </p>
</div>
