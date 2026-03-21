<script lang="ts">
	import { cn } from '$lib/utils/index.js';
	import { type HTMLAttributes } from 'svelte/elements';

	interface Props extends HTMLAttributes<HTMLDivElement> {}

	let { class: className, children, ...rest }: Props = $props();
</script>

<div class={cn('p-6 pt-0', className)} {...rest}>
	{@render children?.()}
</div>
