<script lang="ts">
  import { Upload, Link, Copy, Download, ShieldCheck, Sparkles, TimerReset } from '@lucide/svelte';
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

<main class="relative min-h-screen overflow-hidden bg-slate-950 text-slate-100">
  <div class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(45,212,191,0.28),transparent_34%),radial-gradient(circle_at_85%_20%,rgba(129,140,248,0.24),transparent_30%),linear-gradient(135deg,#020617_0%,#0f172a_52%,#111827_100%)]"></div>
  <div class="pointer-events-none absolute inset-0 opacity-[0.08] [background-image:linear-gradient(rgba(255,255,255,.9)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,.9)_1px,transparent_1px)] [background-size:48px_48px]"></div>

  <div class="relative mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-6 sm:px-6 lg:px-8">
    <header class="flex items-center justify-between py-3">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-2xl border border-cyan-300/25 bg-cyan-300/10 shadow-lg shadow-cyan-950/40">
          <Upload class="h-5 w-5 text-cyan-200" />
        </div>
        <div>
          <p class="text-lg font-black tracking-tight text-white">Folio</p>
          <p class="text-xs font-medium uppercase tracking-[0.32em] text-cyan-200/70">private file drop</p>
        </div>
      </div>
      <div class="hidden items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-slate-300 backdrop-blur md:flex">
        <ShieldCheck class="h-4 w-4 text-emerald-300" />
        Cloudflare Access ready
      </div>
    </header>

    <section class="grid flex-1 items-center gap-8 py-10 lg:grid-cols-[1.04fr_0.96fr] lg:py-16">
      <div class="space-y-8">
        <div class="inline-flex items-center gap-2 rounded-full border border-cyan-300/20 bg-cyan-300/10 px-4 py-2 text-sm font-medium text-cyan-100 shadow-lg shadow-cyan-950/30 backdrop-blur">
          <Sparkles class="h-4 w-4" />
          自架、短連結、過期清理，一次到位
        </div>

        <div class="space-y-5">
          <h1 class="max-w-3xl text-5xl font-black leading-[0.95] tracking-tight text-white sm:text-7xl lg:text-8xl">
            Share files without the SaaS tax.
          </h1>
          <p class="max-w-2xl text-lg leading-8 text-slate-300 sm:text-xl">
            把檔案丟進自己的小型 drop zone，拿到乾淨短連結，需要時再用 Access 保護私密下載。
          </p>
        </div>

        <div class="grid gap-3 sm:grid-cols-3">
          <div class="rounded-3xl border border-white/10 bg-white/[0.06] p-4 backdrop-blur">
            <Upload class="mb-3 h-5 w-5 text-cyan-200" />
            <p class="font-semibold text-white">快速上傳</p>
            <p class="mt-1 text-sm text-slate-400">拖曳或點選都順。</p>
          </div>
          <div class="rounded-3xl border border-white/10 bg-white/[0.06] p-4 backdrop-blur">
            <Link class="mb-3 h-5 w-5 text-violet-200" />
            <p class="font-semibold text-white">短連結</p>
            <p class="mt-1 text-sm text-slate-400">生成後直接分享。</p>
          </div>
          <div class="rounded-3xl border border-white/10 bg-white/[0.06] p-4 backdrop-blur">
            <TimerReset class="mb-3 h-5 w-5 text-amber-200" />
            <p class="font-semibold text-white">自動過期</p>
            <p class="mt-1 text-sm text-slate-400">預設保留 7 天。</p>
          </div>
        </div>
      </div>

      <section aria-labelledby="upload-title" class="rounded-[2rem] border border-white/12 bg-white/[0.08] p-3 shadow-2xl shadow-black/30 backdrop-blur-2xl">
        <div class="rounded-[1.65rem] border border-white/10 bg-slate-950/70 p-5 sm:p-7">
          <div class="mb-6 flex items-start justify-between gap-4">
            <div>
              <p class="text-sm font-semibold uppercase tracking-[0.26em] text-cyan-200/80">drop zone</p>
              <h2 id="upload-title" class="mt-2 text-2xl font-black tracking-tight text-white sm:text-3xl">
                檔案上傳
              </h2>
            </div>
            <div class="rounded-2xl bg-white/10 p-3 text-cyan-200">
              <Upload class="h-5 w-5" />
            </div>
          </div>

          {#if uploadError}
            <div class="mb-5 rounded-2xl border border-rose-300/30 bg-rose-400/10 px-4 py-3 text-sm text-rose-100" role="alert">
              {uploadError}
            </div>
          {/if}

          {#if !uploadedFile}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {:else}
            <div class="space-y-5">
              <div class="rounded-3xl border border-emerald-300/25 bg-emerald-300/10 p-4">
                <div class="flex items-center gap-4">
                  <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-emerald-300/15 text-emerald-200">
                    <Download class="h-5 w-5" />
                  </div>
                  <div class="min-w-0">
                    <h3 class="truncate font-bold text-emerald-50">
                      {uploadedFile.name}
                    </h3>
                    <p class="text-sm text-emerald-100/75">
                      {(uploadedFile.size / 1024 / 1024).toFixed(2)} MB · 已準備分享
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
                class="min-h-11 w-full rounded-2xl border border-white/10 bg-white/[0.07] px-4 py-3 text-sm font-bold text-white transition hover:bg-white/[0.12] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300 cursor-pointer"
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
