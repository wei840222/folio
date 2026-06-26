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

<main class="relative min-h-screen overflow-hidden bg-[#07111f] text-[#f7efe6]">
  <div class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_16%_12%,rgba(141,221,213,0.24),transparent_31%),radial-gradient(circle_at_88%_18%,rgba(217,108,74,0.18),transparent_26%),linear-gradient(135deg,#07111f_0%,#101827_48%,#120c0a_100%)]"></div>
  <div class="pointer-events-none absolute inset-0 opacity-[0.06] [background-image:linear-gradient(rgba(247,239,230,.8)_1px,transparent_1px),linear-gradient(90deg,rgba(247,239,230,.8)_1px,transparent_1px)] [background-size:56px_56px]"></div>
  <div class="pointer-events-none absolute -right-24 top-32 h-80 w-80 rounded-full border border-[#d96c4a]/25 opacity-40"></div>

  <div class="relative mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-6 sm:px-6 lg:px-8">
    <header class="flex items-center justify-between py-3">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-2xl border border-[#8dddd5]/25 bg-[#8dddd5]/10 shadow-lg shadow-black/30">
          <Upload class="h-5 w-5 text-[#8dddd5]" />
        </div>
        <div>
          <p class="text-lg font-black tracking-tight text-[#fff8ee]">Folio</p>
          <p class="text-xs font-medium uppercase tracking-[0.32em] text-[#8dddd5]/75">sealed file drop</p>
        </div>
      </div>
      <div class="hidden items-center gap-2 rounded-full border border-[#f7efe6]/10 bg-[#f7efe6]/5 px-4 py-2 text-sm text-[#d8cfc3] backdrop-blur md:flex">
        <ShieldCheck class="h-4 w-4 text-[#7ee2b8]" />
        Cloudflare Access ready
      </div>
    </header>

    <section class="grid flex-1 items-center gap-8 py-10 lg:grid-cols-[1.04fr_0.96fr] lg:py-16">
      <div class="space-y-8">
        <div class="inline-flex items-center gap-3 rounded-full border border-[#d96c4a]/25 bg-[#d96c4a]/10 px-4 py-2 text-sm font-bold text-[#ffd5c5] shadow-lg shadow-black/25 backdrop-blur">
          <span class="h-2 w-2 rounded-full bg-[#d96c4a]"></span>
          自架投遞 · 私密封存 · 短效分享
        </div>

        <div class="space-y-5">
          <h1 class="max-w-3xl font-serif text-5xl font-black leading-[0.95] tracking-[-0.06em] text-[#fff8ee] sm:text-7xl lg:text-8xl">
            Drop once. Share on your own terms.
          </h1>
          <p class="max-w-2xl text-lg leading-8 text-[#d8cfc3] sm:text-xl">
            把檔案投入自己的小型封存口，取得乾淨短連結；需要時再用 Access 把下載權限鎖好。
          </p>
        </div>

        <div class="grid gap-3 sm:grid-cols-3">
          <div class="rounded-3xl border border-[#f7efe6]/10 bg-[#f7efe6]/[0.055] p-4 backdrop-blur">
            <p class="text-xs font-bold uppercase tracking-[0.24em] text-[#8dddd5]">01 · drop</p>
            <p class="mt-3 font-semibold text-[#fff8ee]">快速投遞</p>
            <p class="mt-1 text-sm text-[#a89f95]">拖曳或點選都順。</p>
          </div>
          <div class="rounded-3xl border border-[#f7efe6]/10 bg-[#f7efe6]/[0.055] p-4 backdrop-blur">
            <p class="text-xs font-bold uppercase tracking-[0.24em] text-[#8dddd5]">02 · link</p>
            <p class="mt-3 font-semibold text-[#fff8ee]">乾淨連結</p>
            <p class="mt-1 text-sm text-[#a89f95]">生成後直接分享。</p>
          </div>
          <div class="rounded-3xl border border-[#d96c4a]/20 bg-[#d96c4a]/[0.075] p-4 backdrop-blur">
            <p class="text-xs font-bold uppercase tracking-[0.24em] text-[#ffb59b]">03 · seal</p>
            <p class="mt-3 font-semibold text-[#fff8ee]">短效封印</p>
            <p class="mt-1 text-sm text-[#c8aaa0]">預設保留 7 天。</p>
          </div>
        </div>
      </div>

      <section aria-labelledby="upload-title" class="rounded-[2rem] border border-[#f7efe6]/12 bg-[#f7efe6]/[0.075] p-3 shadow-2xl shadow-black/35 backdrop-blur-2xl">
        <div class="relative overflow-hidden rounded-[1.65rem] border border-[#f7efe6]/10 bg-[#08111f]/80 p-5 sm:p-7">
          <div class="absolute inset-x-8 top-0 h-1 rounded-b-full bg-[#d96c4a]/70"></div>
          <div class="mb-6 flex items-start justify-between gap-4">
            <div>
              <p class="text-sm font-semibold uppercase tracking-[0.26em] text-[#8dddd5]/80">sealed drop slot</p>
              <h2 id="upload-title" class="mt-2 text-2xl font-black tracking-tight text-[#fff8ee] sm:text-3xl">
                檔案上傳
              </h2>
            </div>
            <div class="rotate-[-6deg] rounded-2xl border border-[#d96c4a]/35 bg-[#d96c4a]/15 px-3 py-2 text-xs font-black uppercase tracking-[0.22em] text-[#ffd5c5]">
              sealed
            </div>
          </div>

          {#if uploadError}
            <div class="mb-5 rounded-2xl border border-[#fca5a5]/30 bg-[#fca5a5]/10 px-4 py-3 text-sm text-[#ffe2e2]" role="alert">
              {uploadError}
            </div>
          {/if}

          {#if !uploadedFile}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {:else}
            <div class="space-y-5">
              <div class="rounded-3xl border border-[#7ee2b8]/25 bg-[#7ee2b8]/10 p-4">
                <div class="flex items-center gap-4">
                  <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-[#7ee2b8]/15 text-[#7ee2b8]">
                    <Download class="h-5 w-5" />
                  </div>
                  <div class="min-w-0">
                    <div class="mb-1 inline-flex rotate-[-2deg] rounded-full border border-[#d96c4a]/35 px-2 py-0.5 text-[10px] font-black uppercase tracking-[0.22em] text-[#ffb59b]">
                      ready seal
                    </div>
                    <h3 class="truncate font-bold text-[#effff8]">
                      {uploadedFile.name}
                    </h3>
                    <p class="text-sm text-[#c8f5e4]">
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
                class="min-h-11 w-full cursor-pointer rounded-2xl border border-[#f7efe6]/10 bg-[#f7efe6]/[0.07] px-4 py-3 text-sm font-bold text-[#fff8ee] transition hover:bg-[#f7efe6]/[0.12] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#8dddd5]"
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
