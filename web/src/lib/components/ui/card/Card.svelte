<script lang="ts">
	import { cn } from '$lib/utils/index.js';
	import { type HTMLAttributes } from 'svelte/elements';

	interface Props extends HTMLAttributes<HTMLDivElement> {}

	let { class: className, children, ...rest }: Props = $props();
</script>

<div class={cn('bg-card text-card-foreground rounded-xl border shadow', className)} {...rest}>
	{@render children?.()}
</div>
