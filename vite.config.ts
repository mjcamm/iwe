import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// @ts-check
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [sveltekit()],

	// Pre-bundle heavy deps so the first page load is fast
	optimizeDeps: {
		include: [
			'yjs',
			'y-prosemirror',
			'@tiptap/core',
			'@tiptap/starter-kit',
			'@tiptap/extension-underline',
			'@tiptap/extension-text-align',
			'@tiptap/extension-color',
			'@tiptap/extension-text-style',
			'@tiptap/extension-superscript',
			'@tiptap/extension-subscript',
			'prosemirror-state',
			'prosemirror-view',
			'prosemirror-model',
			'svelte-dnd-action',
		],
	},

	// Vite dev server config for Tauri
	server: {
		port: 5173,
		strictPort: true,
		host: host || false,
		hmr: host
			? { protocol: 'ws', host, port: 5174 }
			: undefined,
		// Give Vite more time for initial module transforms
		watch: {
			ignored: ['**/src-tauri/**'],
		},
	},
});
