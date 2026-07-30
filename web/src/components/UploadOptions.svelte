<script lang="ts">
  import { Shield, Clock, ChevronDown, ChevronUp, Lock } from '@lucide/svelte';

  let {
    authorizedEmails = $bindable(''),
    expireTtl = $bindable('604800'),
  }: {
    authorizedEmails: string;
    expireTtl: string;
  } = $props();

  let isOpen = $state(false);

  const ttlOptions = [
    { value: '3600', label: '1 小時 (1 Hour)' },
    { value: '86400', label: '24 小時 (1 Day)' },
    { value: '259200', label: '3 天 (3 Days)' },
    { value: '604800', label: '7 天 (7 Days - 預設)' },
    { value: '2592000', label: '30 天 (30 Days)' },
  ];

  function toggleOpen() {
    isOpen = !isOpen;
  }
</script>

<div class="mt-4 rounded-xl border border-primary-border/40 bg-surface/60 overflow-hidden">
  <button
    type="button"
    onclick={toggleOpen}
    class="w-full flex items-center justify-between px-4 py-3 text-xs font-semibold text-text-secondary hover:text-text transition hover:bg-primary-soft/50"
  >
    <div class="flex items-center gap-2">
      <Lock class="h-4 w-4 text-cyber-cyan" />
      <span class="font-mono tracking-wider uppercase text-cyber-cyan">進階設定 / CYBER PROTOCOL OPTIONS</span>
      {#if authorizedEmails.trim() || expireTtl !== '604800'}
        <span class="inline-flex items-center rounded-full bg-cyber-purple/20 px-2 py-0.5 text-[10px] text-cyber-purple border border-cyber-purple/40 font-mono">
          已自訂
        </span>
      {/if}
    </div>
    {#if isOpen}
      <ChevronUp class="h-4 w-4 text-text-muted" />
    {:else}
      <ChevronDown class="h-4 w-4 text-text-muted" />
    {/if}
  </button>

  {#if isOpen}
    <div class="p-4 space-y-4 border-t border-border/40 bg-surface-subtle/80">
      <!-- Email Authorization Matrix -->
      <div>
        <label for="emails-input" class="block mb-1.5 text-xs font-mono text-text-secondary flex items-center gap-1.5">
          <Shield class="h-3.5 w-3.5 text-cyber-emerald" />
          指定存取 Email (Cloudflare Access 授權, 多筆請用逗號分隔)
        </label>
        <input
          id="emails-input"
          type="text"
          bind:value={authorizedEmails}
          placeholder="e.g. alice@company.com, bob@company.com"
          class="w-full rounded-lg border border-border bg-background/80 px-3.5 py-2.5 text-xs text-text font-mono placeholder:text-text-muted/60 focus:border-cyber-cyan focus:outline-none focus:ring-1 focus:ring-cyber-cyan"
        />
        <p class="mt-1 text-[11px] text-text-muted">
          填寫後僅有清單中的 Email 能下載此檔案，未授權用戶將被引導至 Cloudflare Access 驗證。
        </p>
      </div>

      <!-- Expiry Lifecycle TTL -->
      <div>
        <label for="ttl-select" class="block mb-1.5 text-xs font-mono text-text-secondary flex items-center gap-1.5">
          <Clock class="h-3.5 w-3.5 text-cyber-cyan" />
          檔案生命週期 (TTL 到期銷毀時間)
        </label>
        <select
          id="ttl-select"
          bind:value={expireTtl}
          class="w-full rounded-lg border border-border bg-background/80 px-3.5 py-2.5 text-xs text-text font-mono focus:border-cyber-cyan focus:outline-none focus:ring-1 focus:ring-cyber-cyan"
        >
          {#each ttlOptions as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <p class="mt-1 text-[11px] text-text-muted">
          到期後伺服器後端背景 Thread 將自動清除檔案與存取記錄。
        </p>
      </div>
    </div>
  {/if}
</div>
