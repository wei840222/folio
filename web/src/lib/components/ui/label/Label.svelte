<script lang="ts">
	import { cn } from '$lib/utils/index.js';
	import { type HTMLLabelAttributes } from 'svelte/elements';

	interface Props extends HTMLLabelAttributes {}

	let { class: className, children, ...rest }: Props = $props();
</script>

<label
	class={cn(
		'text-sm leading-none font-medium peer-disabled:cursor-not-allowed peer-disabled:opacity-70',
		className
	)}
	{...rest}
>
	{@render children?.()}
</label>
