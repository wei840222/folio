import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		proxy: {
			'^/(uploads|files|private-files)': {
				target: 'http://localhost:8000',
				changeOrigin: true
			}
		}
	}
});
