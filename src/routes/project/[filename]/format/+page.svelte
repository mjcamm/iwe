<script>
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { dndzone } from 'svelte-dnd-action';
  import {
    getChapters, getFormatProfiles, getFormatPages, getSettings, saveSettings,
    seedFormatProfiles, updateFormatProfile, addFormatPage,
    updateFormatPage, deleteFormatPage, reorderFormatPages,
  } from '$lib/db.js';
  import { createChapterDoc, destroyDoc } from '$lib/ydoc.js';
  import { yDocToProsemirrorJSON } from 'y-prosemirror';
  import { generateHTML } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import TextAlign from '@tiptap/extension-text-align';
  import Superscript from '@tiptap/extension-superscript';
  import Subscript from '@tiptap/extension-subscript';

  const flipDurationMs = 150;

  const exportExtensions = [
    StarterKit.configure({ heading: { levels: [1, 2, 3] }, history: false }),
    TextAlign.configure({ types: ['heading', 'paragraph'] }),
    Superscript,
    Subscript,
  ];

  // Target format presets
  const TARGET_PRESETS = {
    print: [
      { label: '6\u00d79 Paperback', w: 6, h: 9 },
      { label: '5.5\u00d78.5 Paperback', w: 5.5, h: 8.5 },
      { label: '5\u00d78 Paperback', w: 5, h: 8 },
      { label: 'A5', w: 5.83, h: 8.27 },
      { label: 'US Letter', w: 8.5, h: 11 },
    ],
    ebook: [
      { label: 'Kindle', w: 4.5, h: 7.2 },
      { label: 'EPUB (generic)', w: 5, h: 7.5 },
      { label: 'Apple Books', w: 5, h: 7.5 },
    ],
  };

  const PAGE_ROLES = [
    'half-title', 'title', 'copyright', 'dedication', 'epigraph', 'toc',
    'foreword', 'preface', 'prologue', 'epilogue', 'afterword',
    'acknowledgments', 'about-author', 'also-by', 'glossary', 'excerpt',
    'blurbs', 'custom',
  ];

  function roleLabel(role) {
    const labels = {
      'half-title': 'Half Title', 'title': 'Title Page', 'copyright': 'Copyright',
      'dedication': 'Dedication', 'epigraph': 'Epigraph', 'toc': 'Table of Contents',
      'foreword': 'Foreword', 'preface': 'Preface', 'prologue': 'Prologue',
      'epilogue': 'Epilogue', 'afterword': 'Afterword', 'acknowledgments': 'Acknowledgments',
      'about-author': 'About the Author', 'also-by': 'Also By', 'glossary': 'Glossary',
      'excerpt': 'Excerpt', 'blurbs': 'Blurbs', 'custom': 'Custom',
    };
    return labels[role] || role;
  }

  // Sidebar modes
  const SIDEBAR_MODES = [
    { key: 'pages', label: 'Pages', icon: 'bi-file-earmark-text' },
    { key: 'target', label: 'Target', icon: 'bi-rulers' },
    { key: 'themes', label: 'Themes', icon: 'bi-palette' },
    { key: 'custom', label: 'Custom', icon: 'bi-sliders' },
  ];

  // Resize
  let sidebarWidth = $state(320);
  let dragging = $state(false);
  let saveWidthTimer = null;
  const SIDEBAR_MIN = 220;
  const SIDEBAR_MAX = 600;

  function persistWidth() {
    clearTimeout(saveWidthTimer);
    saveWidthTimer = setTimeout(async () => {
      const settings = await getSettings();
      settings.formatSidebarWidth = sidebarWidth;
      await saveSettings(settings);
    }, 300);
  }

  function startDrag() {
    dragging = true;
    const onMove = (e) => {
      if (!dragging) return;
      const newWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, window.innerWidth - e.clientX));
      sidebarWidth = newWidth;
    };
    const onUp = () => {
      dragging = false;
      persistWidth();
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  // State
  let loading = $state(true);
  let profiles = $state([]);
  let activeProfileId = $state(null);
  let sidebarMode = $state('pages');
  let chapters = $state([]);
  let chapterHtmls = $state({}); // id -> html string
  let formatPages = $state([]); // FormatPage[] for active profile
  let selectedPresetLabel = $state(null); // tracks which target card is selected

  // Assembled page list for sidebar (front matter + chapters + back matter)
  let pageList = $derived.by(() => {
    const front = formatPages
      .filter(p => p.position === 'front')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, type: 'format-page' }));
    const chapterItems = chapters.map((ch, i) => ({
      id: `ch-${ch.id}`,
      _chapterId: ch.id,
      title: ch.title,
      type: 'chapter',
      sort_order: i,
    }));
    const back = formatPages
      .filter(p => p.position === 'back')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, type: 'format-page' }));
    return [...front, ...chapterItems, ...back];
  });

  // DnD items for front and back sections
  let frontItems = $state([]);
  let backItems = $state([]);

  $effect(() => {
    frontItems = formatPages
      .filter(p => p.position === 'front')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, dndId: p.id }));
  });

  $effect(() => {
    backItems = formatPages
      .filter(p => p.position === 'back')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, dndId: p.id }));
  });

  let activeProfile = $derived(profiles.find(p => p.id === activeProfileId) || null);

  // Page dimensions for preview (in inches)
  let trimWidth = $derived(activeProfile?.trim_width_in ?? 6);
  let trimHeight = $derived(activeProfile?.trim_height_in ?? 9);
  let isEbook = $derived(activeProfile?.target_type === 'ebook');

  // Preview scaling: fit page into available space
  const PREVIEW_SCALE = 96; // pixels per inch for preview
  let pageWidthPx = $derived(trimWidth * PREVIEW_SCALE);
  let pageHeightPx = $derived(trimHeight * PREVIEW_SCALE);

  // Preview scroll ref
  let previewContainer;
  let scrollTarget = $state(null);

  function chapterToHtml(chapter) {
    try {
      const { doc, xmlFragment } = createChapterDoc(chapter.content);
      const json = yDocToProsemirrorJSON(doc, 'prosemirror');
      destroyDoc(doc);
      return generateHTML(json, exportExtensions);
    } catch (e) {
      return '<p>(empty)</p>';
    }
  }

  async function loadData() {
    loading = true;
    // Restore sidebar width
    const settings = await getSettings();
    if (settings.formatSidebarWidth) sidebarWidth = settings.formatSidebarWidth;

    await seedFormatProfiles();
    const [profs, chaps] = await Promise.all([getFormatProfiles(), getChapters()]);
    profiles = profs;
    chapters = chaps;

    // Generate HTML for all chapters
    const htmlMap = {};
    for (const ch of chaps) {
      htmlMap[ch.id] = chapterToHtml(ch);
    }
    chapterHtmls = htmlMap;

    // Select first profile if none selected
    if (!activeProfileId && profs.length > 0) {
      activeProfileId = profs[0].id;
    }

    if (activeProfileId) {
      formatPages = await getFormatPages(activeProfileId);
      syncPresetLabel(profs.find(p => p.id === activeProfileId));
    }
    loading = false;
  }

  function syncPresetLabel(prof) {
    if (!prof) { selectedPresetLabel = null; return; }
    const allPresets = [...TARGET_PRESETS.print, ...TARGET_PRESETS.ebook];
    const match = allPresets.find(t => Math.abs(t.w - prof.trim_width_in) < 0.01 && Math.abs(t.h - prof.trim_height_in) < 0.01);
    selectedPresetLabel = match?.label || null;
  }

  async function switchProfile(id) {
    activeProfileId = id;
    formatPages = await getFormatPages(id);
    syncPresetLabel(profiles.find(p => p.id === id));
  }

  async function selectTarget(preset, type) {
    if (!activeProfileId) return;
    const prof = activeProfile;
    if (!prof) return;
    await updateFormatProfile(
      prof.id, prof.name, type,
      preset.w, preset.h,
      prof.margin_top_in, prof.margin_bottom_in,
      prof.margin_outside_in, prof.margin_inside_in,
      prof.font_body, prof.font_size_pt, prof.line_spacing,
    );
    // Reload profiles
    profiles = await getFormatProfiles();
    selectedPresetLabel = preset.label;
  }

  async function handleAddPage(position) {
    if (!activeProfileId) return;
    const id = await addFormatPage(activeProfileId, 'custom', 'New Page', position);
    formatPages = await getFormatPages(activeProfileId);
  }

  async function handleDeletePage(id) {
    await deleteFormatPage(id);
    formatPages = await getFormatPages(activeProfileId);
  }

  async function handleRoleChange(page, newRole) {
    await updateFormatPage(page.id, newRole, page.title, page.content, page.position, page.include_in);
    formatPages = await getFormatPages(activeProfileId);
  }

  // DnD handlers for front matter
  function handleFrontConsider(e) {
    frontItems = e.detail.items;
  }
  async function handleFrontFinalize(e) {
    frontItems = e.detail.items;
    const ids = frontItems.map(it => it.id);
    await reorderFormatPages(ids);
    formatPages = await getFormatPages(activeProfileId);
  }

  // DnD handlers for back matter
  function handleBackConsider(e) {
    backItems = e.detail.items;
  }
  async function handleBackFinalize(e) {
    backItems = e.detail.items;
    const ids = backItems.map(it => it.id);
    await reorderFormatPages(ids);
    formatPages = await getFormatPages(activeProfileId);
  }

  function scrollToSection(id) {
    scrollTarget = id;
    const el = document.getElementById(`preview-${id}`);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  onMount(loadData);
</script>

{#if loading}
  <div class="format-loading">
    <div class="loader"></div>
    <p>Loading formatting...</p>
  </div>
{:else}
  <div class="format-layout">
    <!-- Center: Page preview -->
    <div class="preview-area" bind:this={previewContainer}>
      <div class="preview-scroll">
        <!-- Front matter pages -->
        {#each pageList as item (item.type === 'chapter' ? item.id : `fp-${item.id}`)}
          {#if item.type === 'format-page'}
            <div class="preview-page-wrap" id="preview-fp-{item.id}">
              <div class="preview-page" class:ebook-frame={isEbook}
                style="width: {pageWidthPx}px; height: {pageHeightPx}px;">
                <div class="page-content format-page-content">
                  <h3 class="page-title">{item.title || roleLabel(item.page_role)}</h3>
                  {#if item.page_role === 'toc'}
                    <div class="toc-placeholder">
                      {#each chapters as ch, i}
                        <div class="toc-entry">
                          <span class="toc-chapter">{ch.title}</span>
                          <span class="toc-dots"></span>
                          <span class="toc-page">{i + 1}</span>
                        </div>
                      {/each}
                    </div>
                  {:else if item.content}
                    <div class="page-text">{item.content}</div>
                  {:else}
                    <div class="page-placeholder">{roleLabel(item.page_role)}</div>
                  {/if}
                </div>
              </div>
              <div class="page-number">{roleLabel(item.page_role)}</div>
            </div>
          {:else}
            <!-- Chapter page -->
            <div class="preview-page-wrap" id="preview-{item.id}">
              <div class="preview-page" class:ebook-frame={isEbook}
                style="width: {pageWidthPx}px; height: {pageHeightPx}px;">
                <div class="page-content">
                  <h2 class="chapter-title">{item.title}</h2>
                  <div class="chapter-body">
                    {@html chapterHtmls[item._chapterId] || '<p>(empty)</p>'}
                  </div>
                </div>
              </div>
              <div class="page-number">{item.title}</div>
            </div>
          {/if}
        {/each}
      </div>
    </div>

    <!-- Resize handle -->
    <div
      class="resize-handle"
      class:active={dragging}
      role="separator"
      aria-orientation="vertical"
      onmousedown={startDrag}
    ></div>

    <!-- Right sidebar -->
    <div class="format-sidebar" style="width: {sidebarWidth}px;">
      <!-- Mode selector -->
      <div class="sidebar-section mode-section">
        <div class="mode-tabs">
          {#each SIDEBAR_MODES as mode}
            <button class="mode-tab" class:active={sidebarMode === mode.key}
              onclick={() => sidebarMode = mode.key}>
              <i class="bi {mode.icon}"></i>
              <span>{mode.label}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Mode content -->
      <div class="sidebar-mode-content">
        {#if sidebarMode === 'pages'}
          <div class="mode-panel page-list-panel">
            <!-- Front matter (draggable) -->
            <div class="page-group-label">
              Front Matter
              <button class="add-page-btn" title="Add front matter page"
                onclick={() => handleAddPage('front')}>
                <i class="bi bi-plus"></i>
              </button>
            </div>
            <div class="dnd-zone"
              use:dndzone={{ items: frontItems, flipDurationMs, type: 'front-pages', dropTargetStyle: { outline: '2px dashed var(--iwe-accent)', 'outline-offset': '-2px' } }}
              onconsider={handleFrontConsider}
              onfinalize={handleFrontFinalize}>
              {#each frontItems as item (item.id)}
                <div class="page-list-item format-page-item" animate:flip={{ duration: flipDurationMs }}
                  onclick={() => scrollToSection(`fp-${item.id}`)}>
                  <i class="bi bi-grip-vertical drag-handle"></i>
                  <span class="page-item-title">{item.title || roleLabel(item.page_role)}</span>
                  <span class="page-item-role">{roleLabel(item.page_role)}</span>
                  <button class="page-item-delete" title="Remove page"
                    onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                    <i class="bi bi-x"></i>
                  </button>
                </div>
              {/each}
            </div>

            <!-- Chapters (locked, non-draggable) -->
            <div class="page-group-label">Chapters</div>
            {#each chapters as ch (ch.id)}
              <div class="page-list-item chapter-item"
                onclick={() => scrollToSection(`ch-${ch.id}`)}>
                <span class="page-item-title">{ch.title}</span>
              </div>
            {/each}

            <!-- Back matter (draggable) -->
            <div class="page-group-label">
              Back Matter
              <button class="add-page-btn" title="Add back matter page"
                onclick={() => handleAddPage('back')}>
                <i class="bi bi-plus"></i>
              </button>
            </div>
            <div class="dnd-zone"
              use:dndzone={{ items: backItems, flipDurationMs, type: 'back-pages', dropTargetStyle: { outline: '2px dashed var(--iwe-accent)', 'outline-offset': '-2px' } }}
              onconsider={handleBackConsider}
              onfinalize={handleBackFinalize}>
              {#each backItems as item (item.id)}
                <div class="page-list-item format-page-item" animate:flip={{ duration: flipDurationMs }}
                  onclick={() => scrollToSection(`fp-${item.id}`)}>
                  <i class="bi bi-grip-vertical drag-handle"></i>
                  <span class="page-item-title">{item.title || roleLabel(item.page_role)}</span>
                  <span class="page-item-role">{roleLabel(item.page_role)}</span>
                  <button class="page-item-delete" title="Remove page"
                    onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                    <i class="bi bi-x"></i>
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {:else if sidebarMode === 'target'}
          <div class="mode-panel">
            <div class="target-group">
              <div class="target-group-label">Print</div>
              {#each TARGET_PRESETS.print as preset}
                <button class="target-card" class:selected={selectedPresetLabel === preset.label}
                  onclick={() => selectTarget(preset, 'print')}>
                  <div class="target-card-icon">
                    <div class="target-thumb print-thumb"
                      style="aspect-ratio: {preset.w} / {preset.h};"></div>
                  </div>
                  <div class="target-card-info">
                    <span class="target-card-label">{preset.label}</span>
                    <span class="target-card-dims">{preset.w}" &times; {preset.h}"</span>
                  </div>
                </button>
              {/each}
            </div>
            <div class="target-group">
              <div class="target-group-label">Ebook</div>
              {#each TARGET_PRESETS.ebook as preset}
                <button class="target-card" class:selected={selectedPresetLabel === preset.label}
                  onclick={() => selectTarget(preset, 'ebook')}>
                  <div class="target-card-icon">
                    <div class="target-thumb ebook-thumb"
                      style="aspect-ratio: {preset.w} / {preset.h};"></div>
                  </div>
                  <div class="target-card-info">
                    <span class="target-card-label">{preset.label}</span>
                    <span class="target-card-dims">{preset.w}" &times; {preset.h}"</span>
                  </div>
                </button>
              {/each}
            </div>
          </div>
        {:else if sidebarMode === 'themes'}
          <div class="mode-panel">
            <p class="shell-placeholder">Theme presets will appear here.</p>
          </div>
        {:else if sidebarMode === 'custom'}
          <div class="mode-panel">
            <p class="shell-placeholder">Custom formatting controls will appear here.</p>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .format-loading {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 1rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .loader {
    width: 28px; height: 28px;
    border: 3px solid var(--iwe-border); border-top-color: var(--iwe-accent);
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Layout */
  .format-layout {
    display: flex; height: 100%; overflow: hidden;
  }

  /* Preview area */
  .preview-area {
    flex: 1; overflow-y: auto; overflow-x: auto;
    background: #e8e4df;
    padding: 2rem;
    display: flex; justify-content: center;
  }
  .preview-scroll {
    display: flex; flex-direction: column; align-items: center; gap: 1.5rem;
  }
  .preview-page-wrap {
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
  }
  .preview-page {
    background: #fff;
    box-shadow: 0 2px 12px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.06);
    overflow: hidden;
    position: relative;
  }
  .preview-page.ebook-frame {
    border-radius: 8px;
    border: 2px solid #333;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  }
  .page-content {
    padding: 0.75in;
    height: 100%;
    overflow: hidden;
    font-family: 'Liberation Serif', 'Georgia', serif;
    font-size: 10px;
    line-height: 1.5;
    color: #222;
  }
  .format-page-content {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    text-align: center;
  }
  .page-title {
    font-family: 'Libre Baskerville', 'Georgia', serif;
    font-size: 16px; font-weight: 400;
    margin: 0 0 1rem 0; color: #222;
  }
  .chapter-title {
    font-family: 'Libre Baskerville', 'Georgia', serif;
    font-size: 14px; font-weight: 400;
    margin: 0 0 1.5rem 0; text-align: center; color: #222;
  }
  .chapter-body {
    font-size: 9px; line-height: 1.5; color: #333;
  }
  .chapter-body :global(p) {
    margin: 0 0 0.5em 0; text-indent: 1.5em;
  }
  .chapter-body :global(p:first-child) { text-indent: 0; }
  .chapter-body :global(h1), .chapter-body :global(h2), .chapter-body :global(h3) {
    font-size: 11px; margin: 1em 0 0.5em; font-family: 'Libre Baskerville', serif;
  }
  .page-placeholder {
    font-family: var(--iwe-font-ui); font-size: 12px;
    color: var(--iwe-text-muted); font-style: italic;
  }
  .page-number {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-align: center;
  }

  /* TOC placeholder */
  .toc-placeholder { width: 100%; text-align: left; margin-top: 1rem; }
  .toc-entry {
    display: flex; align-items: baseline; gap: 0.3rem;
    font-size: 10px; margin-bottom: 0.4rem; color: #333;
  }
  .toc-chapter { white-space: nowrap; }
  .toc-dots {
    flex: 1; border-bottom: 1px dotted #999;
    margin: 0 0.3rem; position: relative; top: -2px;
  }
  .toc-page { white-space: nowrap; font-size: 9px; color: #666; }

  /* Resize handle */
  .resize-handle {
    width: 5px; flex-shrink: 0;
    cursor: col-resize;
    background: var(--iwe-border);
    position: relative;
    transition: background 150ms;
  }
  .resize-handle::after {
    content: '';
    position: absolute;
    top: 0; bottom: 0;
    left: -3px; right: -3px;
  }
  .resize-handle:hover,
  .resize-handle.active {
    background: var(--iwe-accent);
  }

  /* Sidebar */
  .format-sidebar {
    flex-shrink: 0;
    border-left: none;
    background: var(--iwe-bg-warm);
    display: flex; flex-direction: column;
    overflow: hidden;
  }

  .sidebar-section {
    padding: 0.75rem 0.9rem;
    border-bottom: 1px solid var(--iwe-border);
  }
  .sidebar-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.4rem; display: block;
  }

  /* Mode tabs */
  .mode-section { padding: 0.5rem 0.9rem; }
  .mode-tabs {
    display: flex; gap: 2px;
    background: var(--iwe-bg); border-radius: var(--iwe-radius-sm);
    padding: 2px;
  }
  .mode-tab {
    flex: 1; display: flex; flex-direction: column; align-items: center; gap: 2px;
    padding: 0.4rem 0.2rem;
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    border: none; background: none; color: var(--iwe-text-muted);
    cursor: pointer; border-radius: var(--iwe-radius-sm);
    transition: all 150ms;
  }
  .mode-tab i { font-size: 0.9rem; }
  .mode-tab:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .mode-tab.active {
    background: var(--iwe-bg-warm); color: var(--iwe-accent);
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }

  /* Mode content */
  .sidebar-mode-content {
    flex: 1; overflow-y: auto; overflow-x: hidden;
    padding: 0 0.9rem;
  }
  .mode-panel { padding: 0.2rem 0; }
  .page-list-panel { padding: 0; }
  .shell-placeholder {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); font-style: italic;
    text-align: center; padding: 1.5rem 0;
  }

  /* Target cards */
  .target-group { margin-bottom: 0.6rem; }
  .target-group-label {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.05em; font-weight: 600;
    padding: 0.3rem 0; margin-bottom: 0.2rem;
  }
  .target-card {
    display: flex; align-items: center; gap: 0.6rem;
    width: 100%; padding: 0.4rem 0.5rem;
    border: 1px solid transparent; border-radius: var(--iwe-radius-sm);
    background: none; cursor: pointer;
    font-family: var(--iwe-font-ui); text-align: left;
    transition: all 120ms;
  }
  .target-card:hover { background: var(--iwe-bg-hover); }
  .target-card.selected {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
  }
  .target-card-icon {
    width: 28px; display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
  }
  .target-thumb {
    height: 32px; border: 1px solid var(--iwe-border);
    border-radius: 1px; background: #fff;
  }
  .target-thumb.ebook-thumb { border-radius: 3px; border-color: #555; }
  .target-card.selected .target-thumb { border-color: var(--iwe-accent); }
  .target-card-info { display: flex; flex-direction: column; }
  .target-card-label { font-size: 0.78rem; color: var(--iwe-text); }
  .target-card-dims { font-size: 0.65rem; color: var(--iwe-text-muted); }
  .target-card.selected .target-card-label { color: var(--iwe-accent); font-weight: 500; }

  /* Page list */
  .page-group-label {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    padding: 0.5rem 0 0.25rem 0;
    display: flex; align-items: center; justify-content: space-between;
  }
  .add-page-btn {
    border: none; background: none; color: var(--iwe-accent);
    cursor: pointer; padding: 0 0.2rem; font-size: 0.9rem;
    line-height: 1; border-radius: var(--iwe-radius-sm);
    transition: background 100ms;
  }
  .add-page-btn:hover { background: var(--iwe-bg-hover); }

  .dnd-zone { min-height: 4px; }

  .page-list-item {
    display: flex; align-items: center; gap: 0.35rem;
    padding: 0.35rem 0.4rem; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: background 100ms;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
  }
  .page-list-item:hover { background: var(--iwe-bg-hover); }

  .format-page-item .drag-handle {
    color: var(--iwe-text-muted); font-size: 0.85rem;
    cursor: grab; opacity: 0.5;
  }
  .format-page-item:hover .drag-handle { opacity: 1; }

  .page-item-title {
    flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--iwe-text);
  }
  .page-item-role {
    font-size: 0.65rem; color: var(--iwe-text-muted);
    white-space: nowrap;
  }
  .page-item-delete {
    border: none; background: none; color: var(--iwe-text-muted);
    cursor: pointer; padding: 0 0.15rem; font-size: 0.8rem;
    opacity: 0; transition: opacity 100ms;
  }
  .page-list-item:hover .page-item-delete { opacity: 1; }
  .page-item-delete:hover { color: #c0392b; }

  /* Chapter items are visually distinct */
  .chapter-item {
    padding-left: 1.4rem;
    opacity: 0.7;
    cursor: pointer;
  }
  .chapter-item .page-item-title {
    font-style: italic;
  }

</style>
