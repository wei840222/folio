<script lang="ts">
  import { X, Copy, Check, Download, QrCode } from '@lucide/svelte';

  let {
    url,
    onclose,
  }: {
    url: string;
    onclose: () => void;
  } = $props();

  let copied = $state(false);

  // Quick reliable SVG QR code matrix generator using Google Chart API / SVG fallback or pure canvas QR
  const qrImgUrl = $derived(
    `https://api.qrserver.com/v1/create-qr-code/?size=240x240&data=${encodeURIComponent(url)}&color=00f2fe&bgcolor=0d1424`
  );

  async function copyUrl() {
    try {
      await navigator.clipboard.writeText(url);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (err) {
      console.error('Copy failed:', err);
    }
  }

  function downloadQr() {
    const a = document.createElement('a');
    a.href = qrImgUrl;
    a.download = `folio-qr-${Date.now()}.png`;
    a.target = '_blank';
    a.click();
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-background/80 backdrop-blur-md" role="dialog" aria-modal="true">
  <div class="relative w-full max-w-sm rounded-2xl cyber-glass p-6 border border-cyber-cyan/40 shadow-2xl space-y-5 animate-in fade-in zoom-in-95 duration-200">
    <div class="flex items-center justify-between border-b border-border/40 pb-3">
      <div class="flex items-center gap-2">
        <QrCode class="h-5 w-5 text-cyber-cyan" />
        <h3 class="font-mono text-sm font-bold text-text uppercase tracking-wider">CYBER QR MATRIX</h3>
      </div>
      <button
        type="button"
        onclick={onclose}
        class="rounded-lg p-1.5 text-text-muted hover:text-text hover:bg-surface-subtle transition"
        aria-label="關閉"
      >
        <X class="h-4 w-4" />
      </button>
    </div>

    <!-- QR Container -->
    <div class="flex flex-col items-center justify-center p-4 rounded-xl border border-cyber-cyan/30 bg-background/90 relative group">
      <div class="relative p-2 bg-[#0d1424] rounded-lg border border-cyber-cyan/20">
        <img
          src={qrImgUrl}
          alt="QR Code"
          class="w-48 h-48 rounded object-contain"
        />
      </div>
      <p class="mt-3 font-mono text-[11px] text-cyber-cyan/80 text-center truncate max-w-xs px-2">
        {url}
      </p>
    </div>

    <!-- Action Buttons -->
    <div class="grid grid-cols-2 gap-3">
      <button
        type="button"
        onclick={copyUrl}
        class="flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg border border-border bg-surface text-xs font-mono text-text transition hover:border-cyber-cyan hover:bg-primary-soft hover:text-cyber-cyan"
      >
        {#if copied}
          <Check class="h-4 w-4 text-cyber-emerald" />
          已複製
        {:else}
          <Copy class="h-4 w-4" />
          複製連結
        {/if}
      </button>

      <button
        type="button"
        onclick={downloadQr}
        class="flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg border border-cyber-cyan/40 bg-cyber-cyan/10 text-xs font-mono text-cyber-cyan transition hover:bg-cyber-cyan hover:text-on-primary font-bold"
      >
        <Download class="h-4 w-4" />
        下載 QR
      </button>
    </div>
  </div>
</div>
