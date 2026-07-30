<script lang="ts">
  import { Copy, Check, QrCode, Eye, ShieldCheck, Clock } from '@lucide/svelte';

  let {
    url,
    expiresAt,
    filename = '',
    onpreview,
    onshowqr,
  }: {
    url: string;
    expiresAt: number | null;
    filename?: string;
    onpreview?: () => void;
    onshowqr?: () => void;
  } = $props();

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

<div class="space-y-4">
  <div class="rounded-xl border border-cyber-cyan/30 bg-background/80 p-4 space-y-3 shadow-inner">
    <div class="flex items-center justify-between">
      <span class="font-mono text-[10px] font-bold uppercase tracking-wider text-cyber-cyan">
        量子短連結 / QUANTUM LINK OUTPUT
      </span>
      <span class="inline-flex items-center gap-1 text-[10px] font-mono text-cyber-emerald">
        <ShieldCheck class="h-3 w-3" />
        CLOUDFLARE ACCESS PROTECTED
      </span>
    </div>

    <div class="flex items-center gap-2">
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        title={filename ? `下載 ${filename}` : url}
        class="flex-1 truncate rounded-lg border border-cyber-cyan/40 bg-cyber-cyan/10 px-4 py-3 font-mono text-xs font-bold text-cyber-cyan transition hover:border-cyber-cyan hover:bg-cyber-cyan/20 hover:shadow-[0_0_15px_rgba(0,242,254,0.3)]"
      >
        {url}
      </a>
      <button
        type="button"
        onclick={copyToClipboard}
        class="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg border border-border bg-surface text-text-secondary transition hover:border-cyber-cyan hover:bg-cyber-cyan/10 hover:text-cyber-cyan focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-cyber-cyan"
        title="複製連結"
        aria-label="複製到剪貼簿"
      >
        {#if copied}
          <Check class="h-4 w-4 text-cyber-emerald" />
        {:else}
          <Copy class="h-4 w-4" />
        {/if}
      </button>
    </div>

    <!-- Quick Action Tool Buttons for Artifacts -->
    <div class="grid grid-cols-2 gap-2 pt-1">
      {#if onpreview}
        <button
          type="button"
          onclick={onpreview}
          class="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg border border-cyber-purple/40 bg-cyber-purple/10 text-cyber-purple hover:bg-cyber-purple/20 transition text-xs font-mono font-semibold"
        >
          <Eye class="h-3.5 w-3.5" />
          線上預覽 Artifact
        </button>
      {/if}

      {#if onshowqr}
        <button
          type="button"
          onclick={onshowqr}
          class="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg border border-cyber-cyan/40 bg-cyber-cyan/10 text-cyber-cyan hover:bg-cyber-cyan/20 transition text-xs font-mono font-semibold"
        >
          <QrCode class="h-3.5 w-3.5" />
          Cyber QR Code
        </button>
      {/if}
    </div>
  </div>

  <div class="flex items-center justify-between text-xs font-mono text-text-muted px-1">
    <span class="flex items-center gap-1">
      <Clock class="h-3.5 w-3.5 text-cyber-cyan" />
      到期銷毀時間
    </span>
    <span class="text-text-secondary font-semibold">
      {expirationLabel ?? '伺服器預設 (7天)'}
    </span>
  </div>
</div>

