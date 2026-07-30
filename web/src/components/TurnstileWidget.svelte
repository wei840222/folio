<script lang="ts">
  import { onMount } from 'svelte';
  import { loadTurnstileScript } from '../turnstileLoader';

  let {
    siteKey,
    onsuccess,
    onloaderror,
  }: {
    siteKey: string;
    onsuccess: (token: string) => void;
    onloaderror: (message: string) => void;
  } = $props();

  let container: HTMLDivElement;
  let widgetId: string | null = null;
  let isLoading = $state(true);
  let loadFailed = $state(false);
  let destroyed = false;

  async function renderChallenge() {
    isLoading = true;
    loadFailed = false;
    onloaderror('');

    try {
      await loadTurnstileScript();
      if (destroyed || !window.turnstile) return;
      widgetId = window.turnstile.render(container, {
        sitekey: siteKey,
        action: 'upload',
        callback: (token: string) => {
          onsuccess(token);
        },
      });
    } catch (error) {
      if (destroyed) return;
      console.error('Turnstile 載入錯誤:', error);
      loadFailed = true;
      onloaderror('無法載入驗證元件，請重試。');
    } finally {
      if (!destroyed) isLoading = false;
    }
  }

  onMount(() => {
    void renderChallenge();
    return () => {
      destroyed = true;
    };
  });

  export function reset() {
    if (widgetId !== null && window.turnstile) {
      window.turnstile.reset(widgetId);
    }
  }
</script>

{#if isLoading}
  <p class="text-xs text-text-secondary" aria-live="polite">載入驗證中…</p>
{:else if loadFailed}
  <button
    type="button"
    onclick={() => void renderChallenge()}
    class="min-h-11 cursor-pointer rounded-lg border border-primary-border bg-surface px-4 py-2 text-sm font-bold text-primary-hover transition hover:bg-primary-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus"
  >
    重新載入驗證
  </button>
{/if}

<div class:hidden={isLoading || loadFailed} bind:this={container}></div>

<style>
  div {
    display: flex;
    justify-content: center;
  }
</style>
