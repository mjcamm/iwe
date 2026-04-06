<script>
  import { onMount, tick } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import * as Y from 'yjs';
  import { Schema } from '@tiptap/pm/model';
  import { prosemirrorJSONToYDoc } from 'y-prosemirror';
  import {
    parseImportFile,
    getProjectsDir,
    openProject,
    addChapter,
    renameChapter,
    updateChapterContent,
    getChapters,
    deleteChapter
  } from '$lib/db.js';

  let path = $state('');
  let format = $state('');
  let blocks = $state([]); // ImportBlock[]
  let breaks = $state([]); // [{ blockIndex, source, title }]
  let method = $state('auto');
  let loading = $state(true);
  let error = $state('');
  let projectTitle = $state('');
  let importing = $state(false);
  let importMsg = $state('');
  let importProgress = $state(0);
  let importTotal = $state(0);

  // Map blockIndex → break object (for fast lookup in template)
  let breaksByIndex = $derived.by(() => {
    const m = new Map();
    for (const b of breaks) m.set(b.blockIndex, b);
    return m;
  });

  let chapterCount = $derived(breaks.length);

  // Compute "problems" for the strip at top
  let problems = $derived.by(() => {
    if (!breaks.length || !blocks.length) return [];
    const out = [];
    const sorted = [...breaks].sort((a, b) => a.blockIndex - b.blockIndex);
    for (let i = 0; i < sorted.length; i++) {
      const start = sorted[i].blockIndex;
      const end = i + 1 < sorted.length ? sorted[i + 1].blockIndex : blocks.length;
      let words = 0;
      for (let j = start; j < end; j++) {
        words += (blocks[j].text.match(/\S+/g) || []).length;
      }
      if (words < 200) {
        out.push({ blockIndex: start, kind: 'short', msg: `“${sorted[i].title}” is unusually short (${words} words)` });
      } else if (words > 15000) {
        out.push({ blockIndex: start, kind: 'long', msg: `“${sorted[i].title}” is unusually long (${words.toLocaleString()} words) — possibly missed a break` });
      }
      if (!sorted[i].title || sorted[i].title.toLowerCase().startsWith('chapter ') === false && sorted[i].title.length < 2) {
        out.push({ blockIndex: start, kind: 'untitled', msg: `Chapter at block ${start} has no title` });
      }
    }
    return out;
  });

  onMount(async () => {
    const params = $page.url.searchParams;
    path = params.get('path') || '';
    if (!path) {
      error = 'No file specified';
      loading = false;
      return;
    }
    // Use filename (without extension) as default project title
    const base = path.split(/[\\/]/).pop() || 'Imported manuscript';
    projectTitle = base.replace(/\.(docx|epub)$/i, '');
    await runDetection('auto');
  });

  async function runDetection(m) {
    loading = true;
    error = '';
    try {
      const result = await parseImportFile(path, m);
      blocks = result.blocks;
      breaks = result.breaks;
      method = result.method;
      format = result.format;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function onMethodChange(e) {
    const newMethod = e.target.value;
    if (breaks.some(b => b.source === 'manual') && method !== 'auto') {
      if (!confirm('Re-running detection will discard your manual edits. Continue?')) {
        e.target.value = method;
        return;
      }
    }
    await runDetection(newMethod);
  }

  function insertBreakAt(blockIndex) {
    if (breaksByIndex.has(blockIndex)) return;
    const title = (blocks[blockIndex]?.text || '').trim().slice(0, 80) || `Chapter ${breaks.length + 1}`;
    breaks = [...breaks, { blockIndex, source: 'manual', title }].sort((a, b) => a.blockIndex - b.blockIndex);
  }

  function removeBreak(blockIndex) {
    breaks = breaks.filter(b => b.blockIndex !== blockIndex);
  }

  function renameBreak(blockIndex, newTitle) {
    breaks = breaks.map(b => b.blockIndex === blockIndex ? { ...b, title: newTitle } : b);
  }

  // Build a TipTap-compatible PM schema. We only use doc/paragraph/heading/text.
  function makeSchema() {
    return new Schema({
      nodes: {
        doc: { content: 'block+' },
        paragraph: {
          group: 'block',
          content: 'inline*',
          parseDOM: [{ tag: 'p' }],
          toDOM: () => ['p', 0]
        },
        heading: {
          group: 'block',
          attrs: { level: { default: 1 } },
          content: 'inline*',
          parseDOM: [{ tag: 'h1' }, { tag: 'h2' }, { tag: 'h3' }],
          toDOM: (node) => [`h${node.attrs.level}`, 0]
        },
        text: { group: 'inline' }
      }
    });
  }

  function chapterBlocksToYDoc(chapterBlocks, schema) {
    const content = chapterBlocks
      .filter(b => b.text && b.text.trim().length > 0)
      .map(b => {
        if (b.isHeading || /^Heading[123]$/.test(b.style)) {
          return {
            type: 'heading',
            attrs: { level: 1 },
            content: [{ type: 'text', text: b.text }]
          };
        }
        return {
          type: 'paragraph',
          content: [{ type: 'text', text: b.text }]
        };
      });
    if (content.length === 0) {
      content.push({ type: 'paragraph' });
    }
    const json = { type: 'doc', content };
    return prosemirrorJSONToYDoc(schema, json, 'prosemirror');
  }

  async function doImport() {
    if (importing) return;
    if (!projectTitle.trim()) {
      alert('Please enter a project title.');
      return;
    }
    if (breaks.length === 0) {
      alert('No chapter breaks defined.');
      return;
    }
    importing = true;
    importMsg = 'Creating project…';
    try {
      const dir = await getProjectsDir();
      if (!dir) throw new Error('No projects directory configured');
      const filename = projectTitle.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '') + '.iwe';
      const filepath = `${dir}/${filename}`;
      await openProject(filepath);

      // Remove the auto-created Chapter 1 (open_project creates an empty schema only,
      // but listProjects creates one when going through createProject — here we open
      // directly so the DB is empty). We still defensively delete any pre-existing.
      const existing = await getChapters();
      for (const ch of existing) {
        await deleteChapter(ch.id);
      }

      const schema = makeSchema();
      const sorted = [...breaks].sort((a, b) => a.blockIndex - b.blockIndex);
      importTotal = sorted.length;
      importProgress = 0;

      for (let i = 0; i < sorted.length; i++) {
        const start = sorted[i].blockIndex;
        const end = i + 1 < sorted.length ? sorted[i + 1].blockIndex : blocks.length;
        // Skip the heading block itself if it's used as the title
        const title = sorted[i].title || `Chapter ${i + 1}`;
        const headerStart = blocks[start] && blocks[start].text.trim() === title.trim();
        const sliceStart = headerStart ? start + 1 : start;
        const slice = blocks.slice(sliceStart, end);

        importMsg = title;
        importProgress = i + 1;
        await tick();

        const id = await addChapter(title);
        const ydoc = chapterBlocksToYDoc(slice, schema);
        const state = Y.encodeStateAsUpdate(ydoc);
        await updateChapterContent(id, state);
        ydoc.destroy();
      }

      importMsg = 'Done!';
      // Notify main window so it refreshes the project list
      try {
        const { emitTo } = await import('@tauri-apps/api/event');
        await emitTo('main', 'projects-changed', { filename });
      } catch {}
      await getCurrentWindow().close();
    } catch (e) {
      importMsg = '';
      alert('Import failed: ' + e);
      console.error(e);
    } finally {
      importing = false;
    }
  }

  async function cancel() {
    await getCurrentWindow().close();
  }

  function scrollToBlock(idx) {
    const el = document.getElementById('blk-' + idx);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }

  // Ctrl+wheel zoom — goes from full readable down to a tiny "minimap" overview
  let zoom = $state(0.85);
  function onWheel(e) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const delta = e.deltaY > 0 ? -0.05 : 0.05;
    zoom = Math.max(0.15, Math.min(1.6, zoom + delta));
  }
</script>

{#if importing}
  <div class="loading-overlay">
    <div class="loading-card">
      <div class="loading-title">Importing manuscript</div>
      <div class="loading-sub">{importProgress} of {importTotal} chapters</div>
      <div class="loading-bar"><div class="loading-bar-fill" style="width: {importTotal ? (importProgress / importTotal * 100) : 0}%"></div></div>
      <div class="loading-current">{importMsg || ''}</div>
    </div>
  </div>
{/if}

<svelte:head>
  <title>Import manuscript — IWE</title>
</svelte:head>

<div class="wrap">
  <header class="topbar">
    <div class="topbar-left">
      <input class="title-input" type="text" bind:value={projectTitle} placeholder="Project title…" disabled={importing} />
      <span class="format-badge">{format || '...'}</span>
    </div>
    <div class="topbar-mid">
      <label class="meth-label">Detect by:</label>
      <select class="meth-select" value={method} onchange={onMethodChange} disabled={loading || importing}>
        <option value="auto">Auto</option>
        <option value="heading">Heading styles</option>
        <option value="page_break">Page breaks</option>
        <option value="pattern">"Chapter N" pattern</option>
        <option value="blank_lines">Blank-line gaps</option>
      </select>
      <span class="chap-count">{chapterCount} chapter{chapterCount === 1 ? '' : 's'}</span>
      <span class="zoom-hint" title="Hold Ctrl and scroll to zoom">{Math.round(zoom * 100)}%</span>
    </div>
    <div class="topbar-right">
      <button class="btn ghost" onclick={cancel} disabled={importing}>Cancel</button>
      <button class="btn primary" onclick={doImport} disabled={loading || importing || !blocks.length}>
        {importing ? 'Importing…' : `Import ${chapterCount} chapter${chapterCount === 1 ? '' : 's'}`}
      </button>
    </div>
  </header>

  {#if loading}
    <div class="state">Reading document…</div>
  {:else if error}
    <div class="state err">{error}</div>
  {:else}
    {#if problems.length > 0}
      <div class="problems">
        <div class="prob-title">⚠ {problems.length} thing{problems.length === 1 ? '' : 's'} to check</div>
        {#each problems as p}
          <button class="prob-item" onclick={() => scrollToBlock(p.blockIndex)}>{p.msg}</button>
        {/each}
      </div>
    {/if}

    <main
      class="prose"
      style="font-size: {zoom}rem; line-height: {1.45 + zoom * 0.3};"
      onwheel={onWheel}
    >
      {#each blocks as block, i (i)}
        {#if breaksByIndex.has(i)}
          {@const br = breaksByIndex.get(i)}
          <div class="chap-break" id="brk-{i}">
            <div class="chap-line"></div>
            <div class="chap-mid">
              <input
                class="chap-title"
                type="text"
                value={br.title}
                oninput={(e) => renameBreak(i, e.target.value)}
              />
              <span class="chap-source" title="Detected by: {br.source}">{br.source}</span>
              <button class="chap-x" onclick={() => removeBreak(i)} title="Remove break">×</button>
            </div>
            <div class="chap-line"></div>
          </div>
        {:else if i > 0}
          <button class="gap" onclick={() => insertBreakAt(i)} title="Insert chapter break here">
            <span class="gap-line"></span>
            <span class="gap-plus">＋ break here</span>
            <span class="gap-line"></span>
          </button>
        {/if}
        <p
          id="blk-{i}"
          class="block"
          class:heading={block.isHeading}
          class:empty={!block.text.trim()}
        >{block.text || '\u00A0'}</p>
      {/each}
    </main>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    overflow: auto !important; height: auto !important;
    background: #faf8f5;
    margin: 0;
  }
  :global(body) {
    font-family: 'Source Sans 3', system-ui, sans-serif;
    color: #2d2a26;
  }
  .wrap {
    min-height: 100vh;
    display: flex; flex-direction: column;
  }
  .topbar {
    position: sticky; top: 0; z-index: 10;
    display: flex; align-items: center; gap: 1rem;
    padding: 0.7rem 1.2rem;
    background: #fffef9;
    border-bottom: 1px solid #e6e0d6;
    box-shadow: 0 1px 4px rgba(0,0,0,0.03);
  }
  .topbar-left { display: flex; align-items: center; gap: 0.6rem; flex: 1; min-width: 0; }
  .topbar-mid  { display: flex; align-items: center; gap: 0.5rem; }
  .topbar-right { display: flex; align-items: center; gap: 0.5rem; }
  .title-input {
    font-family: 'Libre Baskerville', serif;
    font-size: 1.05rem; font-weight: 400;
    border: none; background: transparent;
    color: #2d2a26; padding: 0.3rem 0.4rem;
    border-bottom: 1px dashed transparent; min-width: 200px;
  }
  .title-input:focus { outline: none; border-bottom-color: #2d6a5e; }
  .format-badge {
    font-size: 0.65rem; text-transform: uppercase; letter-spacing: 0.08em;
    background: #2d6a5e; color: white; padding: 0.15rem 0.45rem; border-radius: 3px;
  }
  .meth-label { font-size: 0.78rem; color: #6b6560; }
  .meth-select {
    font-size: 0.82rem; padding: 0.3rem 0.5rem;
    border: 1px solid #d8d2c4; border-radius: 4px;
    background: white; color: #2d2a26;
  }
  .chap-count {
    font-size: 0.78rem; color: #6b6560;
    padding-left: 0.6rem; border-left: 1px solid #e6e0d6; margin-left: 0.3rem;
  }
  .btn {
    font-size: 0.85rem; padding: 0.45rem 1rem;
    border: none; border-radius: 4px; cursor: pointer;
    font-family: inherit;
  }
  .btn.primary { background: #2d6a5e; color: white; }
  .btn.primary:hover:not(:disabled) { background: #245449; }
  .btn.primary:disabled { opacity: 0.4; cursor: not-allowed; }
  .btn.ghost {
    background: transparent; color: #6b6560;
  }
  .btn.ghost:hover:not(:disabled) { background: #f0ebe0; }

  .state {
    padding: 4rem; text-align: center;
    font-family: 'Libre Baskerville', serif;
    color: #6b6560;
  }
  .state.err { color: #a0403d; }

  .problems {
    background: #fff8e5;
    border-bottom: 1px solid #f0e0a8;
    padding: 0.6rem 1.2rem;
    display: flex; flex-direction: column; gap: 0.25rem;
  }
  .prob-title {
    font-size: 0.78rem; font-weight: 600; color: #8a6500;
    margin-bottom: 0.2rem;
  }
  .prob-item {
    text-align: left; background: none; border: none; cursor: pointer;
    font-size: 0.82rem; color: #6b5500; padding: 0.15rem 0;
    font-family: inherit;
  }
  .prob-item:hover { color: #2d6a5e; text-decoration: underline; }

  .prose {
    width: 100%;
    margin: 0; padding: 2rem 3rem 6rem;
    font-family: 'Libre Baskerville', Georgia, serif;
    color: #3a3530;
  }
  .block {
    margin: 0 0 0.85em;
  }
  .block.heading {
    font-weight: 700; font-size: 1.15em; color: #2d2a26;
    margin-top: 1.4em;
  }
  .block.empty {
    min-height: 0.6em; margin-bottom: 0.4em; opacity: 0.4;
  }
  .loading-overlay {
    position: fixed; inset: 0; z-index: 1000;
    background: rgba(45, 42, 38, 0.55);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(2px);
  }
  .loading-card {
    background: #fffef9;
    border-radius: 8px;
    padding: 1.8rem 2.2rem;
    width: 420px; max-width: 90vw;
    box-shadow: 0 20px 60px rgba(0,0,0,0.25);
    text-align: center;
  }
  .loading-title {
    font-family: 'Libre Baskerville', serif;
    font-size: 1.1rem; color: #2d2a26;
    margin-bottom: 0.3rem;
  }
  .loading-sub {
    font-size: 0.78rem; color: #6b6560; margin-bottom: 1rem;
    font-variant-numeric: tabular-nums;
  }
  .loading-bar {
    height: 6px; background: #f0ebe0; border-radius: 3px;
    overflow: hidden; margin-bottom: 0.7rem;
  }
  .loading-bar-fill {
    height: 100%; background: #2d6a5e;
    transition: width 200ms ease;
  }
  .loading-current {
    font-size: 0.78rem; color: #6b6560;
    font-style: italic;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    min-height: 1.1em;
  }
  .zoom-hint {
    font-size: 0.7rem; color: #6b6560; min-width: 2.5rem;
    text-align: right; font-variant-numeric: tabular-nums;
  }

  .gap {
    display: block; width: 100%;
    background: none; border: none; cursor: pointer;
    padding: 0.3rem 0; margin: 0;
    opacity: 0;
    transition: opacity 120ms;
    font-family: inherit;
  }
  .gap:hover { opacity: 1; }
  .gap-line {
    display: inline-block; vertical-align: middle;
    width: 38%; height: 1px; background: #2d6a5e;
    opacity: 0.4;
  }
  .gap-plus {
    display: inline-block; padding: 0 0.6rem;
    font-size: 0.75rem; color: #2d6a5e;
    font-family: 'Source Sans 3', sans-serif;
  }

  .chap-break {
    display: flex; align-items: center; gap: 0.6rem;
    margin: 2rem 0 1.2rem;
  }
  .chap-line {
    flex: 1; height: 2px; background: #2d6a5e;
    border-radius: 1px;
  }
  .chap-mid {
    display: flex; align-items: center; gap: 0.4rem;
  }
  .chap-title {
    font-family: 'Libre Baskerville', serif;
    font-size: 1.1rem; font-weight: 600;
    color: #2d6a5e;
    border: none; background: transparent;
    text-align: center; min-width: 200px;
    padding: 0.2rem 0.4rem;
    border-bottom: 1px dashed transparent;
  }
  .chap-title:focus {
    outline: none; border-bottom-color: #2d6a5e;
  }
  .chap-source {
    font-size: 0.65rem; text-transform: uppercase;
    letter-spacing: 0.06em; color: #6b6560;
    background: #f0ebe0; padding: 0.1rem 0.4rem; border-radius: 3px;
  }
  .chap-x {
    background: none; border: none; cursor: pointer;
    color: #a0403d; font-size: 1.2rem; line-height: 1;
    padding: 0 0.3rem; opacity: 0.5;
  }
  .chap-x:hover { opacity: 1; }
</style>
