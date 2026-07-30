<script lang="ts">
  import { Download, ShieldCheck, Cpu, Sparkles, Terminal } from '@lucide/svelte';
  import FileUploadZone from './components/FileUploadZone.svelte';
  import DownloadLink from './components/DownloadLink.svelte';
  import TurnstileWidget from './components/TurnstileWidget.svelte';
  import UploadOptions from './components/UploadOptions.svelte';
  import QRCodeModal from './components/QRCodeModal.svelte';
  import ArtifactPreviewModal from './components/ArtifactPreviewModal.svelte';
  import { readExpiresAt } from './uploadResponse';
  import {
    beginChallengeAttempt,
    canCancelChallenge,
    classifyChallengeFailure,
    isCurrentChallengeAttempt,
  } from './uploadRetry';

  let uploadedFile = $state<File | null>(null);
  let isUploading = $state(false);
  let shortUrl = $state('');
  let expiresAt = $state<number | null>(null);
  let uploadError = $state('');
  let showTurnstile = $state(false);
  let turnstileSiteKey = $state('');
  let pendingFile = $state<File | null>(null);
  let turnstileAttempt = $state(0);
  let uploadGeneration = $state(0);

  // Cyber Options
  let authorizedEmails = $state('');
  let expireTtl = $state('604800');

  // Modals
  let showQrModal = $state(false);
  let showPreviewModal = $state(false);

  let artifactBadge = $derived(
    uploadedFile ? getArtifactBadge(uploadedFile.name) : null
  );

  // Helper to construct upload URL with expire query param
  function getUploadEndpoint() {
    return expireTtl ? `/uploads?expire=${encodeURIComponent(expireTtl)}` : '/uploads';
  }

  async function handleFileUpload(file: File) {
    isUploading = true;
    uploadError = '';
    showTurnstile = false;

    const formData = new FormData();
    formData.append('file', file);
    if (authorizedEmails.trim()) {
      formData.append('authorized_emails', authorizedEmails.trim());
    }

    try {
      const res = await fetch(getUploadEndpoint(), {
        method: 'POST',
        body: formData,
      });

      if (res.status === 429) {
        const data = await res.json();
        if (data.code === 'challenge_required' && data.turnstile_site_key) {
          pendingFile = file;
          turnstileSiteKey = data.turnstile_site_key;
          turnstileAttempt += 1;
          showTurnstile = true;
          isUploading = false;
          return;
        }
      }

      if (!res.ok) {
        throw new Error('上傳失敗');
      }

      const location = res.headers.get('Location');
      if (!location) {
        throw new Error('上傳回應缺少下載位置');
      }

      shortUrl = `${window.location.origin}${location}`;
      uploadedFile = file;
      expiresAt = await readExpiresAt(res);
    } catch (error) {
      console.error('上傳錯誤:', error);
      uploadError = '檔案上傳失敗，請稍後再試。';
      return;
    } finally {
      isUploading = false;
    }
  }

  async function handleTurnstileSuccess(token: string) {
    const attempt = beginChallengeAttempt(uploadGeneration, pendingFile);
    if (!attempt) return;

    isUploading = true;
    uploadError = '';

    const formData = new FormData();
    formData.append('file', attempt.file);
    if (authorizedEmails.trim()) {
      formData.append('authorized_emails', authorizedEmails.trim());
    }

    try {
      const res = await fetch(getUploadEndpoint(), {
        method: 'POST',
        body: formData,
        headers: {
          'X-Turnstile-Token': token,
        },
      });

      if (!isCurrentChallengeAttempt(uploadGeneration, attempt)) return;

      if (!res.ok) {
        const body: unknown = await res.json().catch(() => null);
        const code =
          typeof body === 'object' && body !== null && 'code' in body && typeof body.code === 'string'
            ? body.code
            : undefined;
        const failure = classifyChallengeFailure(res.status, code);
        uploadError = failure.message;
        if (failure.retryChallenge) {
          turnstileAttempt += 1;
        } else {
          showTurnstile = false;
          pendingFile = null;
        }
        return;
      }

      const location = res.headers.get('Location');
      if (!location) {
        throw new Error('上傳回應缺少下載位置');
      }

      shortUrl = `${window.location.origin}${location}`;
      uploadedFile = attempt.file;
      expiresAt = await readExpiresAt(res);
      showTurnstile = false;
      pendingFile = null;
    } catch (error) {
      if (!isCurrentChallengeAttempt(uploadGeneration, attempt)) return;
      console.error('驗證後上傳錯誤:', error);
      uploadError = '驗證後上傳失敗，請再完成一次驗證。';
      turnstileAttempt += 1;
      return;
    } finally {
      if (isCurrentChallengeAttempt(uploadGeneration, attempt)) {
        isUploading = false;
      }
    }
  }

  function handleReset() {
    if (!canCancelChallenge(isUploading)) return;
    uploadGeneration += 1;
    uploadedFile = null;
    shortUrl = '';
    expiresAt = null;
    uploadError = '';
    showTurnstile = false;
    pendingFile = null;
    turnstileAttempt = 0;
  }

  function handleTurnstileLoadError(message: string) {
    uploadError = message;
  }

  // Detect artifact file tag based on filename
  function getArtifactBadge(filename: string) {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    if (['md', 'markdown'].includes(ext)) return { label: 'MARKDOWN REPORT', color: 'text-cyber-purple border-cyber-purple/40 bg-cyber-purple/10' };
    if (['py', 'js', 'ts', 'rs', 'html', 'css', 'json', 'sh', 'yaml'].includes(ext)) return { label: 'CODE ARTIFACT', color: 'text-cyber-cyan border-cyber-cyan/40 bg-cyber-cyan/10' };
    if (['png', 'jpg', 'jpeg', 'svg', 'webp', 'gif'].includes(ext)) return { label: 'UI MOCKUP / MEDIA', color: 'text-cyber-emerald border-cyber-emerald/40 bg-cyber-emerald/10' };
    if (['zip', 'tar', 'gz', '7z'].includes(ext)) return { label: 'WORKSPACE ARCHIVE', color: 'text-amber-400 border-amber-400/40 bg-amber-400/10' };
    return { label: 'DATA SPEC', color: 'text-slate-300 border-slate-600 bg-slate-800/50' };
  }
</script>

<svelte:head>
  <title>Folio — Cyber Artifact Portfolio</title>
</svelte:head>

<main class="min-h-screen silicon-bg text-text selection:bg-cyber-cyan selection:text-on-primary">
  <div class="mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-6 sm:px-6 lg:px-8">
    <!-- Header -->
    <header class="flex items-center justify-between py-4 border-b border-border/30">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-xl border border-cyber-cyan/40 bg-cyber-cyan/10 shadow-[0_0_15px_rgba(0,242,254,0.25)]">
          <Cpu class="h-6 w-6 text-cyber-cyan animate-pulse" />
        </div>
        <div>
          <div class="flex items-center gap-2">
            <p class="folio-display text-xl font-extrabold tracking-wider text-text">FOLIO</p>
            <span class="font-mono text-[10px] px-2 py-0.5 rounded bg-cyber-cyan/10 text-cyber-cyan border border-cyber-cyan/30 uppercase">
              Agent Edition
            </span>
          </div>
          <p class="folio-label text-[10px] font-semibold uppercase text-text-secondary tracking-widest">
            DIGITAL ARTIFACT PORTFOLIO
          </p>
        </div>
      </div>

      <div class="hidden items-center gap-3 md:flex">
        <div class="flex items-center gap-2 rounded-full border border-cyber-emerald/40 bg-cyber-emerald/10 px-3.5 py-1.5 text-xs font-mono text-cyber-emerald shadow-[0_0_10px_rgba(16,185,129,0.2)]">
          <span class="h-2 w-2 rounded-full bg-cyber-emerald animate-ping"></span>
          [SYSTEM: NOMINAL]
        </div>
        <div class="flex items-center gap-2 rounded-full border border-cyber-cyan/30 bg-surface px-4 py-1.5 text-xs font-mono text-text-secondary">
          <ShieldCheck class="h-4 w-4 text-cyber-cyan" />
          CLOUDFLARE ACCESS JWT
        </div>
      </div>
    </header>

    <!-- Main Content Section -->
    <section class="grid flex-1 items-center gap-10 py-10 lg:grid-cols-[1.04fr_0.96fr] lg:py-16">
      <!-- Left Hero Info -->
      <div class="space-y-8">
        <div class="inline-flex items-center gap-2.5 rounded-full border border-cyber-cyan/30 bg-cyber-cyan/10 px-4 py-2 text-xs font-mono font-bold text-cyber-cyan shadow-[0_0_20px_rgba(0,242,254,0.15)]">
          <Sparkles class="h-4 w-4 text-cyber-cyan" />
          檔案典藏冊 · AI ARTIFACT 託管 · 私密授權傳輸
        </div>

        <div class="space-y-5">
          <h1 class="folio-display max-w-3xl text-4xl font-extrabold leading-[1.1] text-text sm:text-5xl lg:text-6xl tracking-tight">
            Drop your <span class="text-neon-cyan">Artifacts</span>.<br />
            Shared on your terms.
          </h1>
          <h2 class="text-xl font-mono font-semibold text-cyber-purple">
            &gt;_ 為 AI Agent 與開發者打造的成果典藏冊 (Artifact Portfolio)
          </h2>
          <p class="max-w-2xl text-sm sm:text-base leading-7 text-text-secondary">
            上傳 Markdown 報告、程式碼、UI Mockup 或專案包。可自訂 Email 存取控管與生命週期，點擊即時線上閱讀渲染。
          </p>
        </div>

        <ul class="flex flex-wrap gap-x-6 gap-y-3 font-mono text-xs text-text-secondary" aria-label="服務特性">
          <li class="flex items-center gap-2">
            <span class="h-1.5 w-1.5 rounded-full bg-cyber-cyan"></span>
            支援 Markdown / Code / 全檔案
          </li>
          <li class="flex items-center gap-2">
            <span class="h-1.5 w-1.5 rounded-full bg-cyber-purple"></span>
            Cloudflare Access Email 私密控管
          </li>
          <li class="flex items-center gap-2">
            <span class="h-1.5 w-1.5 rounded-full bg-cyber-emerald"></span>
            自動生命週期 TTL 銷毀
          </li>
        </ul>
      </div>

      <!-- Right Upload Section Card -->
      <section aria-labelledby="upload-title" class="cyber-glass rounded-2xl p-6 sm:p-8 shadow-2xl relative">
        <div class="mb-6 flex items-center justify-between border-b border-border/40 pb-4">
          <div class="flex items-center gap-2">
            <Terminal class="h-5 w-5 text-cyber-cyan" />
            <h2 id="upload-title" class="folio-display text-xl font-bold text-text uppercase tracking-wider">
              UPLINK CORE
            </h2>
          </div>
          <span class="folio-label rounded-full border border-cyber-cyan/40 bg-cyber-cyan/10 px-3 py-1 text-[10px] font-bold uppercase text-cyber-cyan">
            READY
          </span>
        </div>

        {#if uploadError}
          <div class="mb-5 rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-xs font-mono text-red-400" role="alert">
            🚨 {uploadError}
          </div>
        {/if}

        {#if uploadedFile}
          <div class="space-y-5">
            <!-- Upload Success File Card -->
            <div class="rounded-xl border border-cyber-cyan/40 bg-cyber-cyan/10 p-5 relative overflow-hidden">
              <div class="flex items-start gap-4">
                <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-cyber-cyan/20 text-cyber-cyan border border-cyber-cyan/40">
                  <Download class="h-6 w-6" />
                </div>
                <div class="min-w-0 flex-1">
                  {#if artifactBadge}
                    <div class="mb-1.5 inline-flex rounded-full border px-2.5 py-0.5 text-[9px] font-mono font-bold uppercase {artifactBadge.color}">
                      {artifactBadge.label}
                    </div>
                  {/if}
                  <h3 class="truncate font-mono text-sm font-bold text-text" title={uploadedFile.name}>
                    {uploadedFile.name}
                  </h3>
                  <p class="mt-1 text-xs font-mono text-text-secondary">
                    {(uploadedFile.size / 1024 / 1024).toFixed(2)} MB · 傳輸成功
                  </p>
                </div>
              </div>
            </div>

            {#if shortUrl}
              <DownloadLink
                url={shortUrl}
                {expiresAt}
                filename={uploadedFile.name}
                onpreview={() => (showPreviewModal = true)}
                onshowqr={() => (showQrModal = true)}
              />
            {/if}

            <button
              type="button"
              onclick={handleReset}
              class="w-full min-h-11 cursor-pointer rounded-xl border border-border bg-surface px-4 py-3 font-mono text-xs font-bold text-text-secondary transition hover:border-cyber-cyan hover:bg-cyber-cyan/10 hover:text-cyber-cyan focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-cyber-cyan"
            >
              + 上傳新 Artifact 檔案
            </button>
          </div>
        {:else if showTurnstile && turnstileSiteKey}
          <div class="mb-5 rounded-xl border border-cyber-purple/40 bg-cyber-purple/10 px-4 py-5 text-center space-y-4">
            <p class="font-mono text-sm font-bold text-text">需要防護驗證 (CHALLENGE REQUIRED)</p>
            <p class="font-mono text-xs text-text-secondary">請完成以下 Turnstile 人機驗證以繼續上傳</p>
            {#key turnstileAttempt}
              <TurnstileWidget
                siteKey={turnstileSiteKey}
                onsuccess={handleTurnstileSuccess}
                onloaderror={handleTurnstileLoadError}
              />
            {/key}
            <button
              type="button"
              onclick={handleReset}
              disabled={!canCancelChallenge(isUploading)}
              class="mt-4 w-full min-h-11 cursor-pointer rounded-xl border border-border bg-surface px-4 py-3 font-mono text-xs font-bold text-text-secondary transition hover:border-cyber-cyan hover:text-cyber-cyan disabled:cursor-not-allowed disabled:opacity-60"
            >
              {isUploading ? '處理中…' : '取消上傳'}
            </button>
          </div>
        {:else}
          <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          
          <!-- Upload Options Panel -->
          <UploadOptions bind:authorizedEmails bind:expireTtl />
        {/if}
      </section>
    </section>

    <!-- Footer -->
    <footer class="mt-auto py-6 border-t border-border/20 text-center font-mono text-xs text-text-muted flex flex-col sm:flex-row items-center justify-between gap-4">
      <div class="flex items-center gap-2">
        <Cpu class="h-4 w-4 text-cyber-cyan" />
        <span>FOLIO — SECURE DIGITAL ARTIFACT PORTFOLIO</span>
      </div>
      <div>
        <span>POWERED BY RUST 2024 & SVELTE 5</span>
      </div>
    </footer>
  </div>

  <!-- Modals -->
  {#if showQrModal && shortUrl}
    <QRCodeModal url={shortUrl} onclose={() => (showQrModal = false)} />
  {/if}

  {#if showPreviewModal && shortUrl && uploadedFile}
    <ArtifactPreviewModal
      url={shortUrl}
      filename={uploadedFile.name}
      onclose={() => (showPreviewModal = false)}
    />
  {/if}
</main>

