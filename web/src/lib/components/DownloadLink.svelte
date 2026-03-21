<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import { Copy, Link, ExternalLink } from 'lucide-svelte';

	let { url, fileName }: { url: string; fileName: string } = $props();

	let copied = $state(false);

	async function handleCopy() {
		await navigator.clipboard.writeText(url);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}
</script>

<div class="rounded-lg border border-blue-200 bg-blue-50 p-6">
	<div class="mb-4 flex items-center gap-3">
		<div class="flex h-10 w-10 items-center justify-center rounded-full bg-blue-100">
			<Link class="h-5 w-5 text-blue-600" />
		</div>
		<div>
			<h3 class="font-semibold text-blue-800">分享連結已生成</h3>
			<p class="text-sm text-blue-600">您可以透過連結分享檔案。</p>
		</div>
	</div>

	<div class="space-y-3">
		<div>
			<Label for="download-url" class="text-sm font-medium text-gray-700">分享連結</Label>
			<div class="mt-1 flex gap-2">
				<Input id="download-url" type="url" value={url} readonly class="bg-white" />
				<Button
					onclick={handleCopy}
					variant={copied ? 'secondary' : 'outline'}
					size="sm"
					class="shrink-0"
				>
					{#if copied}
						<Copy class="mr-1 h-4 w-4" />
						已複製
					{:else}
						<Copy class="mr-1 h-4 w-4" />
						複製
					{/if}
				</Button>
			</div>
		</div>

		<Button
			onclick={() => window.open(url, '_blank')}
			class="w-full bg-blue-600 text-white hover:bg-blue-700"
		>
			<ExternalLink class="mr-2 h-4 w-4" />
			打開連結
		</Button>
	</div>

	<div class="mt-4 border-t border-blue-200 pt-4">
		<p class="text-xs text-blue-600">連結有效時間：7天</p>
	</div>
</div>
