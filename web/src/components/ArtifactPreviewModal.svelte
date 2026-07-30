<script lang="ts">
  import { X, Copy, Check, FileText, Code, Image as ImageIcon, Eye, ExternalLink, RefreshCw } from '@lucide/svelte';

  let {
    url,
    filename,
    onclose,
  }: {
    url: string;
    filename: string;
    onclose: () => void;
  } = $props();

  let loading = $state(true);
  let content = $state('');
  let fetchError = $state('');
  let copied = $state(false);

  const ext = $derived(filename.split('.').pop()?.toLowerCase() || '');
  const isImage = $derived(['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'].includes(ext));
  const isMarkdown = $derived(['md', 'markdown'].includes(ext));
  const isCode = $derived(['json', 'py', 'js', 'ts', 'rs', 'html', 'css', 'sh', 'bash', 'yaml', 'yml', 'toml', 'sql', 'xml', 'txt'].includes(ext));

  $effect(() => {
    if (!isImage) {
      loadContent();
    } else {
      loading = false;
    }
  });

  async function loadContent() {
    loading = true;
    fetchError = '';
    try {
      const res = await fetch(url);
      if (!res.ok) {
        throw new Error(`無法讀取檔案 (${res.status})`);
      }
      content = await res.text();
    } catch (err: any) {
      fetchError = err.message || '讀取 Artifact 失敗';
    } finally {
      loading = false;
    }
  }

  async function copyContent() {
    try {
      await navigator.clipboard.writeText(content);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (err) {
      console.error('Copy failed:', err);
    }
  }

  // Simple clean markdown parser for rendering AI artifacts without heavy deps
  function renderSimpleMarkdown(raw: string) {
    if (!raw) return '';
    let html = raw
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');

    // Fenced Code Blocks
    html = html.replace(/```([\s\S]*?)```/g, (_match, p1) => {
      const firstLineEnd = p1.indexOf('\n');
      let lang = 'code';
      let codeText = p1;
      if (firstLineEnd !== -1) {
        const potentialLang = p1.substring(0, firstLineEnd).trim();
        if (potentialLang && !potentialLang.includes(' ')) {
          lang = potentialLang;
          codeText = p1.substring(firstLineEnd + 1);
        }
      }
      return `<pre><div class="text-[10px] font-mono text-cyber-cyan uppercase mb-1 border-b border-border/40 pb-1">${lang}</div><code>${codeText}</code></pre>`;
    });

    // Headers
    html = html.replace(/^### (.*$)/gim, '<h3>$1</h3>');
    html = html.replace(/^## (.*$)/gim, '<h2>$1</h2>');
    html = html.replace(/^# (.*$)/gim, '<h1>$1</h1>');

    // Inline Code
    html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

    // Bold & Italic
    html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
    html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');

    // Blockquotes
    html = html.replace(/^&gt; (.*$)/gim, '<blockquote>$1</blockquote>');

    // Lists
    html = html.replace(/^\* (.*$)/gim, '<ul><li>$1</li></ul>');
    html = html.replace(/^- (.*$)/gim, '<ul><li>$1</li></ul>');
    html = html.replace(/<\/ul>\s*<ul>/g, '');

    // Paragraphs
    html = html.split('\n\n').map(p => p.startsWith('<') ? p : `<p>${p}</p>`).join('\n');

    return html;
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-background/85 backdrop-blur-md" role="dialog" aria-modal="true">
  <div class="relative w-full max-w-4xl h-[85vh] flex flex-col rounded-2xl cyber-glass border border-cyber-cyan/40 shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-200">
    
    <!-- Modal Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-border/40 bg-surface/80">
      <div class="flex items-center gap-3 min-w-0">
        {#if isMarkdown}
          <div class="p-2 rounded-lg bg-cyber-purple/10 text-cyber-purple border border-cyber-purple/30">
            <FileText class="h-5 w-5" />
          </div>
        {:else if isImage}
          <div class="p-2 rounded-lg bg-cyber-emerald/10 text-cyber-emerald border border-cyber-emerald/30">
            <ImageIcon class="h-5 w-5" />
          </div>
        {:else if isCode}
          <div class="p-2 rounded-lg bg-cyber-cyan/10 text-cyber-cyan border border-cyber-cyan/30">
            <Code class="h-5 w-5" />
          </div>
        {:else}
          <div class="p-2 rounded-lg bg-slate-800 text-slate-300 border border-slate-700">
            <FileText class="h-5 w-5" />
          </div>
        {/if}

        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span class="font-mono text-xs text-cyber-cyan uppercase tracking-wider">LIVE ARTIFACT INSPECTOR</span>
            <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-subtle text-text-muted border border-border/40 uppercase">
              .{ext || 'RAW'}
            </span>
          </div>
          <h3 class="font-mono text-sm font-bold text-text truncate" title={filename}>
            {filename}
          </h3>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          class="flex items-center gap-1 px-3 py-1.5 rounded-lg border border-border bg-surface text-xs font-mono text-text-secondary hover:text-cyber-cyan hover:border-cyber-cyan/40 transition"
        >
          <ExternalLink class="h-3.5 w-3.5" />
          原始檔
        </a>
        <button
          type="button"
          onclick={onclose}
          class="p-2 rounded-lg text-text-muted hover:text-text hover:bg-surface-subtle transition"
          aria-label="關閉"
        >
          <X class="h-5 w-5" />
        </button>
      </div>
    </div>

    <!-- Modal Content Area -->
    <div class="flex-1 p-6 overflow-y-auto bg-background/60">
      {#if loading}
        <div class="h-full flex flex-col items-center justify-center space-y-3">
          <RefreshCw class="h-8 w-8 text-cyber-cyan animate-spin" />
          <p class="font-mono text-xs text-text-secondary">載入 Artifact 數據中...</p>
        </div>
      {:else if fetchError}
        <div class="p-4 rounded-xl border border-red-500/30 bg-red-500/10 text-red-400 text-xs font-mono">
          <p class="font-bold mb-1">無法載入預覽：</p>
          <p>{fetchError}</p>
        </div>
      {:else if isImage}
        <div class="h-full flex items-center justify-center p-4 bg-[#080d1a] rounded-xl border border-border/40">
          <img src={url} alt={filename} class="max-h-full max-w-full object-contain rounded" />
        </div>
      {:else if isMarkdown}
        <div class="prose-cyber max-w-none font-sans">
          {@html renderSimpleMarkdown(content)}
        </div>
      {:else}
        <!-- Code / Plain Text Viewer -->
        <div class="relative rounded-xl border border-border/50 bg-[#060913] p-4 font-mono text-xs overflow-x-auto">
          <button
            type="button"
            onclick={copyContent}
            class="absolute top-3 right-3 flex items-center gap-1.5 px-2.5 py-1 rounded bg-surface/80 border border-border text-[11px] text-text-secondary hover:text-cyber-cyan transition"
          >
            {#if copied}
              <Check class="h-3.5 w-3.5 text-cyber-emerald" />
              已複製 Code
            {:else}
              <Copy class="h-3.5 w-3.5" />
              複製 Raw
            {/if}
          </button>
          <pre class="text-slate-200 leading-relaxed font-mono whitespace-pre-wrap">{content}</pre>
        </div>
      {/if}
    </div>

    <!-- Modal Footer -->
    <div class="flex items-center justify-between px-6 py-3 border-t border-border/40 bg-surface/80 text-[11px] font-mono text-text-muted">
      <div class="flex items-center gap-2">
        <Eye class="h-3.5 w-3.5 text-cyber-cyan" />
        <span>STATUS: LIVE INSPECTED</span>
      </div>
      <button
        type="button"
        onclick={onclose}
        class="px-4 py-1.5 rounded-lg border border-border bg-surface text-xs font-mono text-text hover:bg-surface-subtle transition"
      >
        關閉預覽
      </button>
    </div>
  </div>
</div>
