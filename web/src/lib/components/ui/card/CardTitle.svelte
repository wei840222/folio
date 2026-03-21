<script lang="ts">
	import { cn } from '$lib/utils/index.js';
	import { type HTMLAttributes } from 'svelte/elements';

	interface Props extends HTMLAttributes<HTMLHeadingElement> {}

	let { class: className, children, ...rest }: Props = $props();
</script>

<h3 class={cn('leading-none font-semibold tracking-tight', className)} {...rest}>
	{@render children?.()}
</h3>
