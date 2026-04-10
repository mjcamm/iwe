<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { getProjectsDir, openProject, getChapters } from '$lib/db.js';
  import { exportDocx, exportTxt, exportHtml, exportPdf } from '$lib/export.js';
  import { generateHTML } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import TextAlign from '@tiptap/extension-text-align';
  import Superscript from '@tiptap/extension-superscript';
  import Subscript from '@tiptap/extension-subscript';
  import { createChapterDoc, destroyDoc } from '$lib/ydoc.js';
  import { yDocToProsemirrorJSON } from 'y-prosemirror';
  import { listen } from '@tauri-apps/api/event';
  import { addToast } from '$lib/toast.js';
  import Toasts from '$lib/components/Toasts.svelte';

  let { data, children } = $props();

  let projectReady = $state(false);
  let projectTitle = $state('');
  let showExportMenu = $state(false);

  // Derive active section from URL
  let activeSection = $derived.by(() => {
    const path = $page.url.pathname;
    const base = `/project/${encodeURIComponent(data.filename)}`;
    if (path.endsWith('/kanban') || path.includes('/kanban/')) return 'kanban';
    if (path.endsWith('/palettes') || path.includes('/palettes/')) return 'palettes';
    if (path.endsWith('/stats') || path.includes('/stats/')) return 'stats';
    if (path.endsWith('/timeflow') || path.includes('/timeflow/')) return 'timeflow';
    if (path.endsWith('/format') || path.includes('/format/')) return 'format';
    if (path.endsWith('/book-settings') || path.includes('/book-settings/')) return 'book-settings';
    return 'editor';
  });

  let basePath = $derived(`/project/${encodeURIComponent(data.filename)}`);

  // Schema extensions for generating HTML from Y.Doc (matches editor config)
  const exportExtensions = [
    StarterKit.configure({ heading: { levels: [1, 2, 3] }, history: false }),
    TextAlign.configure({ types: ['heading', 'paragraph'] }),
    Superscript,
    Subscript,
  ];

  function chapterToHtml(chapterContent) {
    try {
      const { doc, xmlFragment } = createChapterDoc(chapterContent);
      const json = yDocToProsemirrorJSON(doc, 'prosemirror');
      destroyDoc(doc);
      return generateHTML(json, exportExtensions);
    } catch (e) {
      console.warn('[export] failed to generate HTML:', e);
      return '';
    }
  }

  async function handleExport(format) {
    showExportMenu = false;
    try {
      const chapters = await getChapters();
      let path;
      if (format === 'pdf-a4') {
        path = await exportPdf(chapters, projectTitle, 'a4');
      } else if (format === 'pdf-book') {
        path = await exportPdf(chapters, projectTitle, 'book');
      } else {
        const htmlChapters = chapters.map(ch => ({
          ...ch,
          content: chapterToHtml(ch.content),
        }));
        if (format === 'docx') path = await exportDocx(htmlChapters, projectTitle);
        else if (format === 'txt') path = await exportTxt(htmlChapters, projectTitle);
        else if (format === 'html') path = await exportHtml(htmlChapters, projectTitle);
      }
      if (path) addToast(`Exported to ${path.split(/[/\\]/).pop()}`, 'success');
    } catch (e) {
      console.error('Export failed:', e);
      addToast('Export failed: ' + e, 'error');
    }
  }

  onMount(async () => {
    const dir = await getProjectsDir();
    if (!dir) { goto('/'); return; }

    const filepath = `${dir}/${data.filename}`;
    await openProject(filepath);
    projectTitle = data.filename.replace('.iwe', '');
    projectReady = true;

    // Listen for cross-window navigation events (from popup analysis windows)
    listen('navigate-to-position', (event) => {
      const p = event.payload;
      // If we're not on the editor page, navigate there first
      if (activeSection !== 'editor') {
        goto(basePath);
      }
      // Dispatch a custom event that the editor page will pick up
      window.dispatchEvent(new CustomEvent('iwe-navigate-to-position', { detail: p }));
    });
  });

  function closeExportMenu(e) {
    if (showExportMenu) showExportMenu = false;
  }
</script>

<svelte:window onclick={closeExportMenu} />

{#if !projectReady}
  <div class="project-loading">
    <div class="project-loading-inner">
      <div class="loader"></div>
      <p class="loading-text">Loading your manuscript...</p>
    </div>
  </div>
{:else}
  <div class="layout-shell">
    <div class="toolbar">
      <a href="/" class="toolbar-back" title="Back to manuscripts">&larr;</a>
      <span class="toolbar-sep"></span>
      <span class="toolbar-title">{projectTitle}</span>

      <div class="export-wrap" onclick={e => e.stopPropagation()}>
        <button class="export-btn" onclick={() => showExportMenu = !showExportMenu} title="Export manuscript">
          <i class="bi bi-download"></i> Export
        </button>
        {#if showExportMenu}
          <div class="export-menu">
            <button class="export-option" onclick={() => handleExport('docx')}>
              <i class="bi bi-file-earmark-word"></i> Word (.docx)
            </button>
            <button class="export-option" onclick={() => handleExport('pdf-a4')}>
              <i class="bi bi-file-earmark-pdf"></i> PDF — A4
            </button>
            <button class="export-option" onclick={() => handleExport('pdf-book')}>
              <i class="bi bi-book"></i> PDF — Book (5"×8")
            </button>
            <button class="export-option" onclick={() => handleExport('html')}>
              <i class="bi bi-filetype-html"></i> HTML (.html)
            </button>
            <button class="export-option" onclick={() => handleExport('txt')}>
              <i class="bi bi-file-earmark-text"></i> Plain Text (.txt)
            </button>
          </div>
        {/if}
      </div>

      <span class="toolbar-spacer"></span>

      <a href="{basePath}/kanban" class="nav-tab" class:active={activeSection === 'kanban'}>
        <i class="bi bi-kanban"></i> Kanban
      </a>
      <a href={basePath} class="nav-tab" class:active={activeSection === 'editor'}>
        <i class="bi bi-pencil-square"></i> Editor
      </a>
      <a href="{basePath}/palettes" class="nav-tab" class:active={activeSection === 'palettes'}>
        <i class="bi bi-palette2"></i> Palettes
      </a>
      <a href="{basePath}/stats" class="nav-tab" class:active={activeSection === 'stats'}>
        <i class="bi bi-graph-up"></i> Stats
      </a>
      <a href="{basePath}/timeflow" class="nav-tab" class:active={activeSection === 'timeflow'}>
        <i class="bi bi-clock-history"></i> Time Flow
      </a>
      <a href="{basePath}/format" class="nav-tab" class:active={activeSection === 'format'}>
        <i class="bi bi-type"></i> Formatting
      </a>
      <a href="{basePath}/book-settings" class="nav-tab" class:active={activeSection === 'book-settings'}>
        <i class="bi bi-book"></i> Book
      </a>
    </div>

    <div class="layout-content">
      {@render children()}
    </div>
  </div>
{/if}

<Toasts />

<style>
  .project-loading {
    position: fixed; inset: 0; z-index: 10000;
    display: flex; align-items: center; justify-content: center;
    background: var(--iwe-bg-warm);
  }
  .project-loading-inner {
    display: flex; flex-direction: column; align-items: center; gap: 1.2rem;
  }
  .loader {
    width: 32px; height: 32px;
    border: 3px solid var(--iwe-border);
    border-top-color: var(--iwe-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .loading-text {
    font-family: var(--iwe-font-ui);
    color: var(--iwe-text-muted);
    font-size: 0.9rem;
    font-style: italic;
  }

  .layout-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .toolbar {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0 1rem; height: 42px; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
  }
  .toolbar-back {
    text-decoration: none; color: var(--iwe-text-muted);
    font-size: 1.1rem; padding: 0.2rem 0.4rem;
    border-radius: var(--iwe-radius-sm); transition: all 150ms;
  }
  .toolbar-back:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .toolbar-sep { width: 1px; height: 14px; background: var(--iwe-border); }
  .toolbar-title {
    font-family: var(--iwe-font-prose); font-size: 0.95rem;
    color: var(--iwe-text); font-weight: 400;
  }
  .toolbar-spacer { flex: 1; }

  /* Export dropdown — left side, near title */
  .export-wrap { position: relative; }
  .export-btn {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.3rem 0.7rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: inline-flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .export-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .export-menu {
    position: absolute; top: 100%; left: 50%; transform: translateX(-50%);
    margin-top: 4px; z-index: 100;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); padding: 0.3rem;
    box-shadow: 0 6px 20px rgba(0,0,0,0.08);
    display: flex; flex-direction: column; gap: 2px;
    min-width: 170px;
  }
  .export-option {
    display: flex; align-items: center; gap: 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.4rem 0.6rem; border: none; border-radius: var(--iwe-radius-sm);
    background: none; color: var(--iwe-text); cursor: pointer;
    text-align: left; transition: background 100ms;
  }
  .export-option:hover { background: var(--iwe-bg-hover); }
  .export-option i { font-size: 1rem; color: var(--iwe-text-muted); }

  /* Nav tabs — right side */
  .nav-tab {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.3rem 0.7rem; border: 1px solid transparent;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: inline-flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
    text-decoration: none;
  }
  .nav-tab:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .nav-tab.active {
    border-color: var(--iwe-accent);
    color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
    font-weight: 500;
  }

  .layout-content {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
