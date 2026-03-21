<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { Upload, FileText } from 'lucide-svelte';

	let { onFileSelect, isUploading }: { onFileSelect: (file: File) => void; isUploading: boolean } =
		$props();

	let isDragOver = $state(false);

	function handleDragOver(e: DragEvent) {
		e.preventDefault();
		isDragOver = true;
	}

	function handleDragLeave(e: DragEvent) {
		e.preventDefault();
		isDragOver = false;
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault();
		isDragOver = false;

		if (e.dataTransfer && e.dataTransfer.files.length > 0) {
			onFileSelect(e.dataTransfer.files[0]);
		}
	}

	function handleFileInput(e: Event) {
		const target = e.target as HTMLInputElement;
		if (target.files && target.files.length > 0) {
			onFileSelect(target.files[0]);
		}
	}

	let fileInputRef: HTMLInputElement | undefined = $state();
</script>

{#if isUploading}
	<div class="rounded-lg border-2 border-dashed border-blue-300 bg-blue-50 p-12 text-center">
		<div
			class="mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-3 border-blue-600 border-t-transparent"
		></div>
		<h3 class="mb-2 text-lg font-semibold text-blue-800">上傳中...</h3>
		<p class="text-blue-600">請稍後，檔案處理中...</p>
	</div>
{:else}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="cursor-pointer rounded-lg border-2 border-dashed p-12 text-center transition-all duration-200 {isDragOver
			? 'scale-105 border-blue-500 bg-blue-50'
			: 'border-gray-300 hover:border-blue-400 hover:bg-gray-50'}"
		ondragover={handleDragOver}
		ondragleave={handleDragLeave}
		ondrop={handleDrop}
		onclick={() => fileInputRef?.click()}
	>
		<div class="space-y-4">
			<div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-blue-100">
				<Upload class="h-8 w-8 text-blue-600" />
			</div>

			<div>
				<h3 class="mb-2 text-lg font-semibold text-gray-900">拖曳檔案到此處或點選上傳按鈕</h3>
				<p class="mb-4 text-gray-600">支援所有檔案類型，最大大小5MB</p>
			</div>

			<Button type="button" class="bg-blue-600 text-white hover:bg-blue-700">
				<FileText class="mr-2 h-4 w-4" />
				選擇檔案
			</Button>
		</div>

		<input
			bind:this={fileInputRef}
			type="file"
			class="hidden"
			onchange={handleFileInput}
			accept="*/*"
		/>
	</div>
{/if}
