<script lang="ts">
  import { Upload, Link, Copy, Download } from '@lucide/svelte';
  import FileUploadZone from './components/FileUploadZone.svelte';
  import DownloadLink from './components/DownloadLink.svelte';

  let uploadedFile = $state<File | null>(null);
  let isUploading = $state(false);
  let shortUrl = $state('');

  async function handleFileUpload(file: File) {
    isUploading = true;

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
      alert('檔案上傳失敗，請稍後再試。');
      return;
    } finally {
      isUploading = false;
    }
  }

  function handleReset() {
    uploadedFile = null;
    shortUrl = '';
  }
</script>

<svelte:head>
  <title>Folio</title>
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-blue-50 via-white to-purple-50">
  <div class="container mx-auto px-4 py-8 flex flex-col justify-center min-h-screen">
    <div class="max-w-2xl mx-auto">
      <!-- Header -->
      <div class="text-center mb-8">
        <h1 class="text-4xl font-bold text-gray-900 mb-4">Folio</h1>
        <p class="text-lg text-gray-600">
          上傳您的檔案，生成短網址，輕鬆分享給朋友或同事。
        </p>
      </div>

      <!-- Main Card -->
      <div class="bg-white/80 backdrop-blur-sm rounded-xl border py-6 shadow-xl">
        <div class="px-6 pb-6 text-center">
          <h2 class="flex items-center justify-center gap-2 text-2xl font-semibold">
            <Upload class="w-6 h-6 text-blue-600" />
            檔案上傳
          </h2>
        </div>
        <div class="px-6">
          {#if !uploadedFile}
            <FileUploadZone onfileselect={handleFileUpload} {isUploading} />
          {:else}
            <div class="space-y-6">
              <!-- Upload Success Info -->
              <div class="bg-green-50 border border-green-200 rounded-lg p-4">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 bg-green-100 rounded-full flex items-center justify-center">
                    <Download class="w-5 h-5 text-green-600" />
                  </div>
                  <div>
                    <h3 class="font-semibold text-green-800">
                      {uploadedFile.name}
                    </h3>
                    <p class="text-sm text-green-600">
                      檔案大小：{(uploadedFile.size / 1024 / 1024).toFixed(2)} MB
                    </p>
                  </div>
                </div>
              </div>

              <!-- Download Link -->
              {#if shortUrl}
                <DownloadLink url={shortUrl} />
              {/if}

              <!-- Reset Button -->
              <button
                type="button"
                onclick={handleReset}
                class="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md border bg-white shadow-xs px-4 py-2 text-sm font-medium transition-all hover:bg-gray-50 w-full cursor-pointer"
              >
                上傳新檔案
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Features -->
      <div class="grid md:grid-cols-3 gap-6 mt-12">
        <div class="text-center">
          <div class="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center mx-auto mb-3">
            <Upload class="w-6 h-6 text-blue-600" />
          </div>
          <h3 class="font-semibold mb-2">快速上傳</h3>
          <p class="text-sm text-gray-600">支援拖曳上傳，方便快速。</p>
        </div>
        <div class="text-center">
          <div class="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center mx-auto mb-3">
            <Link class="w-6 h-6 text-purple-600" />
          </div>
          <h3 class="font-semibold mb-2">短網址生成</h3>
          <p class="text-sm text-gray-600">自動生成方便分享的短網址。</p>
        </div>
        <div class="text-center">
          <div class="w-12 h-12 bg-green-100 rounded-lg flex items-center justify-center mx-auto mb-3">
            <Copy class="w-6 h-6 text-green-600" />
          </div>
          <h3 class="font-semibold mb-2">一鍵複製</h3>
          <p class="text-sm text-gray-600">點選就能複製分享連結。</p>
        </div>
      </div>
    </div>
  </div>
</div>
