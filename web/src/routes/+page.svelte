<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Upload, Link, Copy, Download } from 'lucide-svelte';
	import FileUploadZone from '$lib/components/FileUploadZone.svelte';
	import DownloadLink from '$lib/components/DownloadLink.svelte';

	let uploadedFile = $state<File | null>(null);
	let isUploading = $state(false);
	let shortUrl = $state<string>('');

	async function handleFileUpload(file: File) {
		isUploading = true;

		const formData = new FormData();
		formData.append('file', file);

		try {
			const res = await fetch('/uploads', {
				method: 'POST',
				body: formData
			});
			if (!res.ok) {
				throw new Error('上傳失敗');
			}
			shortUrl = `${window.location.origin}${res.headers.get('Location')}`;
		} catch (error) {
			console.error('上傳錯誤:', error);
			alert('檔案上傳失敗，請稍後再試。');
			return;
		} finally {
			isUploading = false;
			uploadedFile = file;
		}
	}

	function handleReset() {
		uploadedFile = null;
		shortUrl = '';
	}
</script>

<div class="min-h-screen bg-gradient-to-br from-blue-50 via-white to-purple-50">
	<div class="container mx-auto flex min-h-screen flex-col justify-center px-4 py-8">
		<div class="mx-auto w-full max-w-2xl">
			<!-- Header -->
			<div class="mb-8 text-center">
				<h1 class="mb-4 text-4xl font-bold text-gray-900">Folio</h1>
				<p class="text-lg text-gray-600">上傳您的檔案，生成短網址，輕鬆分享給朋友或同事。</p>
			</div>

			<!-- Main Card -->
			<Card class="border-0 bg-white/80 shadow-xl backdrop-blur-sm">
				<CardHeader class="pb-6 text-center">
					<CardTitle class="flex items-center justify-center gap-2 text-2xl">
						<Upload class="h-6 w-6 text-blue-600" />
						檔案上傳
					</CardTitle>
				</CardHeader>
				<CardContent class="space-y-6">
					{#if !uploadedFile}
						<FileUploadZone onFileSelect={handleFileUpload} {isUploading} />
					{:else}
						<div class="space-y-6">
							<!-- Upload Success Info -->
							<div class="rounded-lg border border-green-200 bg-green-50 p-4">
								<div class="flex items-center gap-3">
									<div class="flex h-10 w-10 items-center justify-center rounded-full bg-green-100">
										<Download class="h-5 w-5 text-green-600" />
									</div>
									<div>
										<h3 class="font-semibold text-green-800">
											{uploadedFile.name}
										</h3>
										<p class="text-sm text-green-600">
											檔案大小：
											{(uploadedFile.size / 1024 / 1024).toFixed(2)} MB
										</p>
									</div>
								</div>
							</div>

							<!-- Download Link -->
							{#if shortUrl}
								<DownloadLink url={shortUrl} fileName={uploadedFile.name} />
							{/if}

							<!-- Reset Button -->
							<Button onclick={handleReset} variant="outline" class="w-full">上傳新檔案</Button>
						</div>
					{/if}
				</CardContent>
			</Card>

			<!-- Features -->
			<div class="mt-12 grid gap-6 md:grid-cols-3">
				<div class="text-center">
					<div
						class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-lg bg-blue-100"
					>
						<Upload class="h-6 w-6 text-blue-600" />
					</div>
					<h3 class="mb-2 font-semibold">快速上傳</h3>
					<p class="text-sm text-gray-600">支援拖曳上傳，方便快速。</p>
				</div>
				<div class="text-center">
					<div
						class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-lg bg-purple-100"
					>
						<Link class="h-6 w-6 text-purple-600" />
					</div>
					<h3 class="mb-2 font-semibold">短網址生成</h3>
					<p class="text-sm text-gray-600">自動生成方便分享的短網址。</p>
				</div>
				<div class="text-center">
					<div
						class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-lg bg-green-100"
					>
						<Copy class="h-6 w-6 text-green-600" />
					</div>
					<h3 class="mb-2 font-semibold">一鍵複製</h3>
					<p class="text-sm text-gray-600">點選就能複製分享連結。</p>
				</div>
			</div>
		</div>
	</div>
</div>
