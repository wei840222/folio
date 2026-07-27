<script lang="ts">
  import { Upload, Download, ShieldCheck } from '@lucide/svelte';
  import FileUploadZone from './components/FileUploadZone.svelte';
  import DownloadLink from './components/DownloadLink.svelte';

  let uploadedFile = $state<File | null>(null);
  let isUploading = $state(false);
  let shortUrl = $state('');
  let uploadError = $state('');

  async function handleFileUpload(file: File) {
    isUploading = true;
    uploadError = '';

    const formData = new FormData();
    formData.append('file', file);

    try {
      const res = await fetch('/uploads', {
        method: 'POST',
        body: formData,
      });
      if (!res.ok) {
        throw new Error('上傳失敗');
      }

      const location = res.headers.get('Location');
      if (!location) {
        throw new Error('上傳回應缺少下載位置');
      }

      shortUrl = `${window.location.origin}${location}`;
      uploadedFile = file;
    } catch (error) {
      console.error('上傳錯誤:', error);
      uploadError = '檔案上傳失敗，請稍後再試。';
      return;
    } finally {
      isUploading = false;
    }
  }

  function handleReset() {
    uploadedFile = null;
    shortUrl = '';
    uploadError = '';
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
          <li class="before:mr-5 before:text-primary before:content-['•']">預設保留 7 天</li>
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

          {#if !uploadedFile}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {:else}
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
                <DownloadLink url={shortUrl} />
              {/if}

              <button
                type="button"
                onclick={handleReset}
                class="min-h-11 w-full cursor-pointer rounded-lg border border-border bg-surface px-4 py-3 text-sm font-bold text-text-secondary transition hover:border-primary-border hover:bg-primary-soft hover:text-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-focus"
              >
                上傳新檔案
              </button>
            </div>
          {/if}
      </section>
    </section>
  </div>
</main>
