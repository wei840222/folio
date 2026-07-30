<script lang="ts">
  import { Upload, Download, ShieldCheck } from '@lucide/svelte';
  import FileUploadZone from './components/FileUploadZone.svelte';
  import DownloadLink from './components/DownloadLink.svelte';
  import TurnstileWidget from './components/TurnstileWidget.svelte';
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


  async function handleFileUpload(file: File) {
    isUploading = true;
    uploadError = '';
    showTurnstile = false;

    const formData = new FormData();
    formData.append('file', file);

    try {
      const res = await fetch('/uploads', {
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

    try {
      const res = await fetch('/uploads', {
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
</script>

<svelte:head>
  <title>Folio</title>
</svelte:head>

<main class="min-h-screen bg-background text-text">
  <div class="mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-6 sm:px-6 lg:px-8">
    <header class="flex items-center justify-between py-3">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-xl border border-primary-border bg-primary-soft">
          <Upload class="h-5 w-5 text-primary" />
        </div>
        <div>
          <p class="folio-display text-lg font-bold text-text">Folio</p>
          <p class="folio-label text-xs font-semibold uppercase text-primary">安全檔案分享</p>
        </div>
      </div>
      <div class="hidden items-center gap-2 rounded-full border border-border-subtle bg-surface px-4 py-2 text-sm text-text-secondary md:flex">
        <ShieldCheck class="h-4 w-4 text-on-success-surface" />
        支援 Cloudflare Access
      </div>
    </header>

    <section class="grid flex-1 items-center gap-10 py-10 lg:grid-cols-[1.04fr_0.96fr] lg:py-16">
      <div class="space-y-8">
        <div class="inline-flex items-center gap-3 rounded-full border border-primary-border bg-primary-soft px-4 py-2 text-sm font-bold text-primary-hover">
          <span class="h-2 w-2 rounded-full bg-primary"></span>
          自架託管 · 私密分享 · 短效連結
        </div>

        <div class="space-y-5">
          <h1 class="folio-display max-w-3xl text-5xl font-bold leading-[1.05] text-text sm:text-6xl lg:text-7xl">
            Drop once.<br />Share on your own terms.
          </h1>
          <h2 class="mt-6 text-2xl font-bold text-text-secondary">
            安全又快速的檔案分享
          </h2>
          <p class="max-w-2xl text-lg leading-8 text-text-secondary sm:text-xl">
            上傳檔案、取得短連結，需要時還能用 email 名單控管存取權限。
          </p>
        </div>

        <ul class="flex flex-wrap gap-x-5 gap-y-2 text-sm text-text-secondary" aria-label="服務特性">
          <li>支援所有檔案類型</li>
          <li class="before:mr-5 before:text-primary before:content-['•']">檔案大小上限 25MB</li>
          <li class="before:mr-5 before:text-primary before:content-['•']">到期時間由伺服器設定</li>
        </ul>
      </div>

      <section aria-labelledby="upload-title" class="rounded-upload border border-primary-border bg-surface-subtle p-5 shadow-sm sm:p-7">
          <div class="mb-6 flex items-start justify-between gap-4">
            <div>
              <h2 id="upload-title" class="folio-display text-3xl font-bold text-text">檔案上傳</h2>
            </div>
            <div class="folio-label rounded-full border border-primary-border bg-primary-soft px-3 py-2 text-xs font-bold uppercase text-primary-hover">
              share
            </div>
          </div>

          {#if uploadError}
            <div class="mb-5 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700" role="alert">
              {uploadError}
            </div>
          {/if}

          {#if uploadedFile}
            <div class="space-y-5">
              <div class="rounded-xl border border-success-border bg-success-surface p-4">
                <div class="flex items-center gap-4">
                  <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-surface text-on-success-surface">
                    <Download class="h-5 w-5" />
                  </div>
                  <div class="min-w-0">
                    <div class="folio-label mb-1 inline-flex rounded-full border border-primary-border bg-primary-soft px-2 py-0.5 text-[10px] font-bold uppercase text-primary-hover">
                      ready
                    </div>
                    <h3 class="truncate font-bold text-text">
                      {shortUrl.split('/').pop()}
                    </h3>
                    <p class="text-sm text-text-secondary">
                      {(uploadedFile.size / 1024 / 1024).toFixed(2)} MB · 已準備分享
                    </p>
                    <p class="mt-0.5 truncate text-xs text-text-muted" title={uploadedFile.name}>
                      原始檔名：{uploadedFile.name}
                    </p>
                  </div>
                </div>
              </div>

              {#if shortUrl}
                <DownloadLink url={shortUrl} {expiresAt} />
              {/if}

              <button
                type="button"
                onclick={handleReset}
                class="min-h-11 w-full cursor-pointer rounded-lg border border-border bg-surface px-4 py-3 text-sm font-bold text-text-secondary transition hover:border-primary-border hover:bg-primary-soft hover:text-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus"
              >
                上傳新檔案
              </button>
            </div>
          {:else if showTurnstile && turnstileSiteKey}
            <div class="mb-5 rounded-lg border border-primary-border bg-primary-soft px-4 py-3 text-center">
              <p class="mb-3 text-sm font-semibold text-text">需要額外驗證</p>
              <p class="mb-4 text-xs text-text-secondary">請完成以下驗證以繼續上傳</p>
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
                class="mt-4 min-h-11 w-full cursor-pointer rounded-lg border border-border bg-surface px-4 py-3 text-sm font-bold text-text-secondary transition hover:border-primary-border hover:bg-primary-soft hover:text-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus disabled:cursor-not-allowed disabled:opacity-60"
              >
                {isUploading ? '上傳中…' : '取消上傳'}
              </button>
            </div>
          {:else}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {/if}
      </section>
    </section>
  </div>
</main>
