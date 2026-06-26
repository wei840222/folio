<script lang="ts">
  import { Upload, Download, ShieldCheck, Mail, Stamp, Link2, Clock } from '@lucide/svelte';
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

<main class="relative min-h-screen overflow-hidden">
  <!-- 信封格線背景 -->
  <div class="absolute inset-0 envelope-grid"></div>
  
  <!-- 裝飾性圓圈 -->
  <div class="pointer-events-none absolute -right-32 top-24 h-64 w-64 rounded-full border border-blue-200/40 opacity-60"></div>
  <div class="pointer-events-none absolute -left-20 bottom-32 h-48 w-48 rounded-full border border-blue-300/30 opacity-40"></div>

  <div class="relative mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-6 sm:px-6 lg:px-8">
    <header class="flex items-center justify-between py-3">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-2xl border border-blue-600/20 bg-blue-100/50 shadow-sm">
          <Upload class="h-5 w-5 text-blue-600" />
        </div>
        <div>
          <p class="text-lg font-black tracking-tight text-slate-900" style="font-family: 'Fraunces', serif;">Folio</p>
          <p class="text-xs font-medium uppercase tracking-[0.32em] text-blue-600/70">安全檔案分享</p>
        </div>
      </div>
      <div class="hidden items-center gap-2 rounded-full border border-slate-200/60 bg-white/80 px-4 py-2 text-sm text-slate-600 backdrop-blur md:flex">
        <ShieldCheck class="h-4 w-4 text-emerald-600" />
        支援 Cloudflare Access
      </div>
    </header>

    <section class="grid flex-1 items-center gap-8 py-10 lg:grid-cols-[1.04fr_0.96fr] lg:py-16">
      <div class="space-y-8">
        <div class="inline-flex items-center gap-3 rounded-full border border-blue-600/20 bg-blue-100/30 px-4 py-2 text-sm font-bold text-blue-700">
          <span class="h-2 w-2 rounded-full bg-blue-600"></span>
          自架託管 · 私密分享 · 短效連結
        </div>

        <div class="space-y-5">
          <h1 class="max-w-3xl text-5xl font-black leading-[0.95] tracking-[-0.06em] text-slate-900 sm:text-7xl lg:text-8xl" style="font-family: 'Fraunces', serif;">
            Drop once.<br />Share on your own terms.
          </h1>
          <h2 class="mt-6 text-2xl font-bold text-slate-700" style="font-family: 'Fraunces', serif;">
            安全又快速的檔案分享
          </h2>
          <p class="max-w-2xl text-lg leading-8 text-slate-600 sm:text-xl">
            上傳檔案、取得短連結，需要時還能用 email 名單控管存取權限。
          </p>
        </div>

        <div class="grid gap-3 sm:grid-cols-3">
          <div class="rounded-2xl border border-slate-200/60 bg-white/70 p-5 backdrop-blur-sm shadow-sm">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-blue-100/60">
              <Upload class="h-5 w-5 text-blue-600" />
            </div>
            <h3 class="text-lg font-bold text-slate-900">快速上傳</h3>
            <p class="mt-1 text-sm text-slate-500">拖曳或點選，怎麼傳都行。</p>
          </div>
          <div class="rounded-2xl border border-slate-200/60 bg-white/70 p-5 backdrop-blur-sm shadow-sm">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-blue-100/60">
              <Link2 class="h-5 w-5 text-blue-600" />
            </div>
            <h3 class="text-lg font-bold text-slate-900">乾淨連結</h3>
            <p class="mt-1 text-sm text-slate-500">產生短網址，直接丟給對方。</p>
          </div>
          <div class="rounded-2xl border border-blue-200/60 bg-blue-50/50 p-5 backdrop-blur-sm shadow-sm">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-blue-100/60">
              <Clock class="h-5 w-5 text-blue-600" />
            </div>
            <h3 class="text-lg font-bold text-slate-900">自動過期</h3>
            <p class="mt-1 text-sm text-slate-500">預設 7 天後自動清除。</p>
          </div>
        </div>
      </div>

      <section aria-labelledby="upload-title" class="rounded-[2rem] border border-slate-200/60 bg-white/80 p-3 shadow-xl shadow-blue-900/5 backdrop-blur-sm">
        <div class="relative overflow-hidden rounded-[1.65rem] border border-slate-200/60 bg-gradient-to-b from-blue-50/40 to-white p-5 sm:p-7">
          <!-- Top accent line -->
          <div class="absolute inset-x-8 top-0 h-1 rounded-b-full bg-blue-600/60"></div>

          <div class="mb-6 flex items-start justify-between gap-4">
            <div>
              <p class="text-sm font-semibold uppercase tracking-[0.26em] text-blue-600/80" style="font-family: 'JetBrains Mono', monospace;">上傳區域</p>
              <h2 id="upload-title" class="mt-2 text-2xl font-black tracking-tight text-slate-900 sm:text-3xl" style="font-family: 'Fraunces', serif;">
                檔案上傳
              </h2>
            </div>
            <div class="rounded-2xl border border-blue-600/20 bg-blue-100/50 px-3 py-2 text-xs font-black uppercase tracking-[0.22em] text-blue-700">
              sealed
            </div>
          </div>

          {#if uploadError}
            <div class="mb-5 rounded-2xl border border-red-200/60 bg-red-50/50 px-4 py-3 text-sm text-red-700" role="alert">
              {uploadError}
            </div>
          {/if}

          {#if !uploadedFile}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {:else}
            <div class="space-y-5">
              <div class="rounded-3xl border border-emerald-200/60 bg-emerald-50/50 p-4">
                <div class="flex items-center gap-4">
                  <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-emerald-100/60 text-emerald-600">
                    <Download class="h-5 w-5" />
                  </div>
                  <div class="min-w-0">
                    <div class="mb-1 inline-flex rounded-full border border-blue-600/20 bg-blue-100/50 px-2 py-0.5 text-[10px] font-black uppercase tracking-[0.22em] text-blue-700">
                      ready
                    </div>
                    <h3 class="truncate font-bold text-slate-900">
                      {shortUrl.split('/').pop()}
                    </h3>
                    <p class="text-sm text-slate-600">
                      {(uploadedFile.size / 1024 / 1024).toFixed(2)} MB · 已準備分享
                    </p>
                    <p class="text-xs text-slate-400 truncate mt-0.5" title={uploadedFile.name}>
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
                class="min-h-11 w-full cursor-pointer rounded-2xl border border-slate-200/80 bg-white px-4 py-3 text-sm font-bold text-slate-700 transition hover:bg-slate-50 hover:border-blue-300 hover:text-blue-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              >
                上傳新檔案
              </button>
            </div>
          {/if}
        </div>
      </section>
    </section>
  </div>
</main>
