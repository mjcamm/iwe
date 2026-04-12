<script>
	import 'bootstrap/dist/css/bootstrap.min.css';
	import 'bootstrap-icons/font/bootstrap-icons.css';
	import '$lib/theme.css';
	import { getSettings } from '$lib/db.js';
	import favicon from '$lib/assets/favicon.svg';

	// Apply the user's UI scale from settings.json as early as possible. Runs
	// once when any webview window mounts — home, project, or popups. Everything
	// downstream uses `rem` so setting the root font-size cascades through the
	// whole interface (menus, sidebars, editor, modals) in one go.
	if (typeof document !== 'undefined') {
		getSettings().then((s) => {
			// Tolerate both number (current) and string (older) storage.
			const scale = Number(s.uiScale) || 1;
			document.documentElement.style.fontSize = (14 * scale) + 'px';
		}).catch(() => {});
	}

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
