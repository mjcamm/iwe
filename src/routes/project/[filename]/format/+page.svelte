<script>
  import { onMount, onDestroy } from 'svelte';
  import { afterNavigate } from '$app/navigation';
  import { flip } from 'svelte/animate';
  import { dndzone } from 'svelte-dnd-action';
  import {
    getChapters, getFormatProfiles, getFormatPages, getSettings, saveSettings,
    seedFormatProfiles, addFormatProfile, updateFormatProfile, deleteFormatProfile,
    duplicateFormatProfile, pasteFormatProfileSettings, addFormatPage,
    updateFormatPage, deleteFormatPage, reorderFormatPages,
    addPageExclusion, removePageExclusion, listPageExclusions,
    compilePreview, exportFormatPdf, getProjectSetting, setProjectSetting,
    updateProfileCategory,
  } from '$lib/db.js';
  import { save } from '@tauri-apps/plugin-dialog';
  import { addToast } from '$lib/toast.js';
  import PageContentEditor from '$lib/components/PageContentEditor.svelte';
  import ChapterHeadings from '$lib/components/format/ChapterHeadings.svelte';
  import ParagraphSettings from '$lib/components/format/ParagraphSettings.svelte';
  import HeadingsSettings from '$lib/components/format/HeadingsSettings.svelte';
  import BreaksSettings from '$lib/components/format/BreaksSettings.svelte';
  import PrintLayoutSettings from '$lib/components/format/PrintLayoutSettings.svelte';
  import TypographySettings from '$lib/components/format/TypographySettings.svelte';
  import HeaderFooterSettings from '$lib/components/format/HeaderFooterSettings.svelte';
  import TrimSettings from '$lib/components/format/TrimSettings.svelte';

  const flipDurationMs = 150;


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

  // Roles that can be applied via the tag bar (custom is the default for the + buttons)
  const ASSIGNABLE_ROLES = PAGE_ROLES.filter(r => r !== 'custom');

  // Custom mode sub-tabs (each is a settings component)
  const CUSTOM_TABS = [
    { id: 'chapter-headings', label: 'Chapter Headings', icon: 'bi-bookmark' },
    { id: 'paragraph',        label: 'Paragraph',        icon: 'bi-paragraph' },
    { id: 'headings',         label: 'Headings',         icon: 'bi-type-h1' },
    { id: 'breaks',           label: 'Breaks',           icon: 'bi-asterisk' },
    { id: 'print-layout',     label: 'Print Layout',     icon: 'bi-layout-text-window' },
    { id: 'typography',       label: 'Typography',       icon: 'bi-fonts' },
    { id: 'header-footer',    label: 'Header / Footer',  icon: 'bi-distribute-vertical' },
    { id: 'trim',             label: 'Trim',             icon: 'bi-aspect-ratio' },
  ];
  let customTab = $state('chapter-headings');
  let customSelectorOpen = $state(false);
  let activeCustomTab = $derived(CUSTOM_TABS.find(t => t.id === customTab) || CUSTOM_TABS[0]);

  function selectCustomTab(id) {
    customTab = id;
    customSelectorOpen = false;
  }

  // Sidebar modes
  const SIDEBAR_MODES = [
    { key: 'pages', label: 'Pages', icon: 'bi-file-earmark-text' },
    { key: 'themes', label: 'Themes', icon: 'bi-palette' },
    { key: 'custom', label: 'Custom', icon: 'bi-sliders' },
    { key: 'export', label: 'Export', icon: 'bi-download' },
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
  let formatPages = $state([]); // ALL project-level pages
  let exclusions = $state([]); // [{ page_id, profile_id }]

  // Profile management UI state
  let showProfileMenu = $state(false);
  let showCreateProfileModal = $state(false);
  let newProfileName = $state('');
  let newProfileDuplicateFrom = $state(null); // profile id or null
  let newProfileTargetType = $state('print');
  let renamingProfileId = $state(null);
  let renameValue = $state('');
  let confirmDeleteProfileId = $state(null);

  // Clipboard for copy/paste profile settings (persists in settings.json)
  let formatClipboard = $state(null); // raw profile object snapshot, or null

  // Helper: is a page included in a given profile?
  function isPageIncludedIn(pageId, profileId) {
    return !exclusions.some(e => e.page_id === pageId && e.profile_id === profileId);
  }

  // Tag-bar state for adding tagged pages
  let armedTag = $state(null);
  let usedTags = $derived(new Set(formatPages.map(p => p.page_role)));

  // Inline rename state for format pages
  let editingPageId = $state(null);
  let editingPageTitle = $state('');

  // Full content editor modal state
  let designingPage = $state(null); // FormatPage being edited, or null

  // Re-attach observer whenever the page count or compile generation changes
  $effect(() => {
    // Reactive deps
    pageCount;
    compileGeneration;
    // Wait for DOM to render new placeholders before observing
    queueMicrotask(() => setupObserver());
  });

  // Compiled preview state
  let pageCount = $state(0);
  let rendering = $state(false); // true during compile
  let renderError = $state(null);
  let lastTiming = $state(null); // CompileTiming from Rust
  let compileGeneration = $state(0); // incremented on each compile to bust img cache

  // Export state
  let exporting = $state(false);
  let exportError = $state(null);
  let sectionPages = $state({}); // section_id -> 0-based page index

  // Lazy-load state — IntersectionObserver tracks visible pages, scroll-idle commits loads
  let loadedPages = $state(new Set()); // page indices that have had src set
  let visibleSet = new Set(); // currently intersecting (not reactive — used by handlers only)
  let pageObserver = null;
  let scrollIdleTimer = null;
  let scrolling = false;
  const SCROLL_IDLE_MS = 150;
  const VISIBLE_BUFFER = 2; // load 2 pages above and below the visible range

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
  let isEbook = $derived(activeProfile?.target_type === 'ebook');

  // Preview scroll ref
  let previewContainer;

  // ---- Lazy loading ----

  function teardownObserver() {
    if (pageObserver) {
      pageObserver.disconnect();
      pageObserver = null;
    }
    visibleSet.clear();
  }

  function setupObserver() {
    teardownObserver();
    if (!previewContainer || pageCount === 0) return;

    pageObserver = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const idx = Number(entry.target.dataset.pageIndex);
        if (entry.isIntersecting) {
          visibleSet.add(idx);
        } else {
          visibleSet.delete(idx);
        }
      }
      // If not currently scrolling, commit immediately (initial load case)
      if (!scrolling) {
        scheduleCommit();
      }
    }, {
      root: previewContainer,
      // Use a large rootMargin so pages just outside viewport count as "visible"
      // and we get a smooth experience without waiting for them to fully enter
      rootMargin: '200px 0px 200px 0px',
      threshold: 0,
    });

    // Observe all current page placeholders
    const elements = previewContainer.querySelectorAll('[data-page-index]');
    for (const el of elements) {
      pageObserver.observe(el);
    }
  }

  function scheduleCommit() {
    clearTimeout(scrollIdleTimer);
    scrollIdleTimer = setTimeout(commitVisible, SCROLL_IDLE_MS);
  }

  function commitVisible() {
    scrolling = false;
    if (visibleSet.size === 0) return;

    // Compute the range we want loaded: visible pages + buffer
    const sorted = [...visibleSet].sort((a, b) => a - b);
    const min = Math.max(0, sorted[0] - VISIBLE_BUFFER);
    const max = Math.min(pageCount - 1, sorted[sorted.length - 1] + VISIBLE_BUFFER);

    const updated = new Set(loadedPages);
    let changed = false;
    for (let i = min; i <= max; i++) {
      if (!updated.has(i)) {
        updated.add(i);
        changed = true;
      }
    }
    if (changed) {
      loadedPages = updated;
    }

    // Persist current scroll position for the active profile so a recompile
    // (settings change) can restore the user's exact viewport.
    persistScrollPosition();
  }

  // ---- Scroll position persistence ----
  let scrollSaveTimer = null;
  function persistScrollPosition() {
    if (!activeProfileId || !previewContainer) return;
    clearTimeout(scrollSaveTimer);
    const top = previewContainer.scrollTop;
    scrollSaveTimer = setTimeout(() => {
      setProjectSetting(`format_scroll_${activeProfileId}`, String(top)).catch(() => {});
    }, 200);
  }

  async function loadSavedScroll(profileId) {
    try {
      const v = await getProjectSetting(`format_scroll_${profileId}`);
      const n = Number(v);
      return Number.isFinite(n) && n >= 0 ? n : 0;
    } catch {
      return 0;
    }
  }

  /// Restore scroll position synchronously (no animation) after a render cycle.
  /// Called from compileAndShow once placeholders are in the DOM.
  function restoreScroll(top) {
    if (!previewContainer || top == null) return;
    previewContainer.scrollTop = top;
  }

  function handlePreviewScroll() {
    scrolling = true;
    scheduleCommit();
  }

  async function compileAndShow() {
    if (!activeProfileId) return;
    if (isEbook) {
      pageCount = 0;
      lastTiming = null;
      teardownObserver();
      loadedPages = new Set();
      return;
    }

    // Snapshot the current scroll position so we can restore it after recompile.
    // Fall back to the saved position from project_settings (used on first load
    // and after switching profiles).
    const liveScroll = previewContainer ? previewContainer.scrollTop : null;
    const savedScroll = await loadSavedScroll(activeProfileId);
    const restoreTo = liveScroll && liveScroll > 0 ? liveScroll : savedScroll;

    rendering = true;
    renderError = null;
    try {
      const result = await compilePreview(activeProfileId);
      pageCount = result.page_count;
      lastTiming = result.timing;
      sectionPages = result.section_pages || {};
      compileGeneration++; // bust img cache
      loadedPages = new Set(); // reset — pages will be loaded lazily by observer

      console.log(
        `[format] Compile: ${result.timing.total_ms.toFixed(0)}ms | ` +
        `db:${result.timing.db_load_ms.toFixed(0)} ydoc:${result.timing.ydoc_extract_ms.toFixed(0)} ` +
        `markup:${result.timing.markup_build_ms.toFixed(0)} compile:${result.timing.typst_compile_ms.toFixed(0)} | ` +
        `${result.page_count} pages, ${result.timing.chapter_count} chapters`
      );
    } catch (e) {
      console.error('[format] compile failed:', e);
      renderError = String(e);
      pageCount = 0;
      lastTiming = null;
    } finally {
      rendering = false;
    }

    // Wait two animation frames so:
    // 1) Svelte commits the new pageCount → placeholders mount
    // 2) Browser computes layout (fixed-height placeholders give correct scroll height)
    // Then jump scroll to restore position before the user sees the top.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        restoreScroll(restoreTo);
      });
    });
  }

  async function reloadPages() {
    [formatPages, exclusions] = await Promise.all([getFormatPages(), listPageExclusions()]);
  }

  // Used by Custom mode settings components after they save a category JSON.
  // Refreshes the local profiles array so the prop they receive is up-to-date,
  // then recompiles the preview.
  async function handleCustomSettingChange() {
    profiles = await getFormatProfiles();
    compileAndShow();
  }

  async function loadData() {
    loading = true;
    // Restore sidebar width and clipboard
    const settings = await getSettings();
    if (settings.formatSidebarWidth) sidebarWidth = settings.formatSidebarWidth;
    if (settings.formatClipboard) formatClipboard = settings.formatClipboard;

    await seedFormatProfiles();
    const [profs, chaps] = await Promise.all([getFormatProfiles(), getChapters()]);
    profiles = profs;
    chapters = chaps;

    // Select first profile if none selected
    if (!activeProfileId && profs.length > 0) {
      activeProfileId = profs[0].id;
    }

    await reloadPages();
    loading = false;

    // Kick off rendering after load
    compileAndShow();
  }

  async function switchProfile(id) {
    activeProfileId = id;
    compileAndShow();
  }

  async function handleAddPage(position) {
    const role = armedTag || 'custom';
    const title = armedTag ? roleLabel(armedTag) : 'New Page';
    await addFormatPage(role, title, position);
    await reloadPages();
    armedTag = null;
    compileAndShow();
  }

  // ---- Tag bar handlers ----

  function armTag(role) {
    if (usedTags.has(role)) return; // already used; X removes it
    armedTag = armedTag === role ? null : role;
  }

  async function deleteTaggedPage(role) {
    const page = formatPages.find(p => p.page_role === role);
    if (page) {
      await deleteFormatPage(page.id);
      await reloadPages();
      compileAndShow();
    }
  }

  async function placeArmedTagBelow(targetItem) {
    if (!armedTag) return;
    const role = armedTag;
    armedTag = null; // disarm immediately so subsequent clicks don't re-fire
    const title = roleLabel(role);
    const newId = await addFormatPage(role, title, targetItem.position);
    await reloadPages();

    // Reorder the section so the new page lands directly below the target
    const section = formatPages
      .filter(p => p.position === targetItem.position)
      .sort((a, b) => a.sort_order - b.sort_order);
    const orderedIds = section.map(p => p.id).filter(id => id !== newId);
    const targetIdx = orderedIds.indexOf(targetItem.id);
    if (targetIdx >= 0) {
      orderedIds.splice(targetIdx + 1, 0, newId);
      await reorderFormatPages(orderedIds);
      await reloadPages();
    }
    compileAndShow();
  }

  function handlePageItemClick(item) {
    if (armedTag) {
      placeArmedTagBelow(item);
    } else {
      scrollToSection(`iwe-fp-${item.id}`);
    }
  }

  function handleChapterItemClick(ch) {
    if (armedTag) {
      // Chapters can't be retagged — just disarm so the user knows nothing happened
      armedTag = null;
      return;
    }
    scrollToSection(`iwe-ch-${ch.id}`);
  }

  function handleEscape(e) {
    if (e.key === 'Escape') {
      if (armedTag) armedTag = null;
      if (editingPageId !== null) editingPageId = null;
    }
  }

  function startEditPage(item) {
    editingPageId = item.id;
    editingPageTitle = item.title || '';
  }

  async function commitEditPage() {
    if (editingPageId === null) return;
    const id = editingPageId;
    const newTitle = editingPageTitle.trim();
    const page = formatPages.find(p => p.id === id);
    editingPageId = null;
    if (!page) return;
    if (newTitle === (page.title || '')) return; // no change
    await updateFormatPage(id, page.page_role, newTitle, page.content, page.position, page.include_in, page.vertical_align);
    await reloadPages();
    compileAndShow();
  }

  function openDesignEditor(item) {
    designingPage = item;
  }

  async function saveDesignedPage({ content: jsonContent, verticalAlign }) {
    if (!designingPage) return;
    const p = designingPage;
    designingPage = null;
    await updateFormatPage(p.id, p.page_role, p.title, jsonContent, p.position, p.include_in, verticalAlign);
    await reloadPages();
    compileAndShow();
  }

  function cancelDesignedPage() {
    designingPage = null;
  }

  async function handleDeletePage(id) {
    await deleteFormatPage(id);
    await reloadPages();
    compileAndShow();
  }

  async function handleRoleChange(page, newRole) {
    await updateFormatPage(page.id, newRole, page.title, page.content, page.position, page.include_in, page.vertical_align);
    await reloadPages();
  }

  // Profile management
  function openCreateProfileModal() {
    newProfileName = '';
    newProfileDuplicateFrom = null;
    newProfileTargetType = 'print';
    showCreateProfileModal = true;
    showProfileMenu = false;
  }

  async function createProfile() {
    const name = newProfileName.trim();
    if (!name) return;

    let newId;
    if (newProfileDuplicateFrom) {
      newId = await duplicateFormatProfile(newProfileDuplicateFrom, name);
    } else {
      // Default sizes per target type
      const w = newProfileTargetType === 'ebook' ? 6.0 : 6.0;
      const h = newProfileTargetType === 'ebook' ? 9.0 : 9.0;
      newId = await addFormatProfile(name, newProfileTargetType, w, h);
    }
    profiles = await getFormatProfiles();
    showCreateProfileModal = false;
    activeProfileId = newId;
    await reloadPages();
    compileAndShow();
  }

  async function copyProfileSettings() {
    if (!activeProfile) return;
    // Snapshot the entire active profile object — Rust filters excluded fields on paste
    formatClipboard = { ...activeProfile };
    // Persist to settings.json so the clipboard survives reloads + project switches
    const settings = await getSettings();
    settings.formatClipboard = formatClipboard;
    await saveSettings(settings);
    addToast(`Copied settings from "${activeProfile.name}"`, 'success');
  }

  async function pasteProfileSettings() {
    if (!activeProfile || !formatClipboard) return;
    if (formatClipboard.id === activeProfile.id) {
      addToast('Cannot paste a profile onto itself', 'info');
      return;
    }
    try {
      await pasteFormatProfileSettings(activeProfile.id, formatClipboard);
      profiles = await getFormatProfiles();
      addToast(`Pasted settings into "${activeProfile.name}"`, 'success');
      compileAndShow();
    } catch (e) {
      console.error('[format] paste failed:', e);
      addToast('Paste failed: ' + e, 'error');
    }
  }

  async function deleteActiveProfile() {
    if (profiles.length <= 1) return;
    const remainingProfile = profiles.find(p => p.id !== activeProfileId);
    await deleteFormatProfile(activeProfileId);
    profiles = await getFormatProfiles();
    activeProfileId = remainingProfile?.id ?? null;
    confirmDeleteProfileId = null;
    showProfileMenu = false;
    compileAndShow();
  }

  function startRenameActive() {
    if (!activeProfile) return;
    renamingProfileId = activeProfile.id;
    renameValue = activeProfile.name;
    showProfileMenu = false;
  }

  async function commitRename() {
    if (!renamingProfileId) return;
    const prof = profiles.find(p => p.id === renamingProfileId);
    if (!prof) { renamingProfileId = null; return; }
    const name = renameValue.trim() || prof.name;
    await updateFormatProfile(
      prof.id, name, prof.target_type,
      prof.trim_width_in, prof.trim_height_in,
      prof.margin_top_in, prof.margin_bottom_in,
      prof.margin_outside_in, prof.margin_inside_in,
      prof.font_body, prof.font_size_pt, prof.line_spacing,
    );
    profiles = await getFormatProfiles();
    renamingProfileId = null;
  }

  // Toggle a page's inclusion in a profile
  async function togglePageInProfile(pageId, profileId) {
    const isIncluded = isPageIncludedIn(pageId, profileId);
    if (isIncluded) {
      await addPageExclusion(pageId, profileId);
    } else {
      await removePageExclusion(pageId, profileId);
    }
    exclusions = await listPageExclusions();
    // Recompile if the change affects the active profile
    if (profileId === activeProfileId) {
      compileAndShow();
    }
  }

  // DnD handlers for front matter
  function handleFrontConsider(e) {
    frontItems = e.detail.items;
  }
  async function handleFrontFinalize(e) {
    frontItems = e.detail.items;
    const ids = frontItems.map(it => it.id);
    await reorderFormatPages(ids);
    await reloadPages();
    compileAndShow();
  }

  // DnD handlers for back matter
  function handleBackConsider(e) {
    backItems = e.detail.items;
  }
  async function handleBackFinalize(e) {
    backItems = e.detail.items;
    const ids = backItems.map(it => it.id);
    await reorderFormatPages(ids);
    await reloadPages();
    compileAndShow();
  }

  async function handleExportPdf() {
    if (exporting || pageCount === 0) return;
    exporting = true;
    exportError = null;
    try {
      // Ensure we have a fresh compile
      if (!pageCount) await compileAndShow();

      const pdfBytes = await exportFormatPdf();
      const arr = pdfBytes instanceof Uint8Array ? pdfBytes : new Uint8Array(pdfBytes);

      // Save dialog
      const profileName = activeProfile?.name || 'export';
      const filePath = await save({
        title: 'Save PDF',
        defaultPath: `${profileName}.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });

      if (filePath) {
        const { writeFile } = await import('@tauri-apps/plugin-fs');
        await writeFile(filePath, arr);
        addToast(`PDF exported to ${filePath.split(/[/\\]/).pop()}`, 'success');
      }
    } catch (e) {
      console.error('[format] export failed:', e);
      exportError = String(e);
      addToast('Export failed: ' + e, 'error');
    } finally {
      exporting = false;
    }
  }

  function scrollToPage(index) {
    const el = document.getElementById(`preview-page-${index}`);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function scrollToSection(sectionId) {
    const idx = sectionPages[sectionId];
    if (idx == null) return;
    scrollToPage(idx);
  }

  let hasLoaded = false;
  onMount(loadData);

  // Recompile whenever the user navigates TO this page (e.g. after editing
  // chapter images/content in the editor tab). afterNavigate fires on every
  // route transition including the initial load — we skip the first one since
  // onMount already handles it.
  afterNavigate(() => {
    if (hasLoaded) {
      compileAndShow();
    }
    hasLoaded = true;
  });

  onDestroy(() => {
    teardownObserver();
    clearTimeout(scrollIdleTimer);
  });
</script>

<svelte:window onkeydown={handleEscape} />

{#if loading}
  <div class="format-loading">
    <div class="loader"></div>
    <p>Loading formatting...</p>
  </div>
{:else}
  <div class="format-layout">
    <!-- Center: Preview + timing -->
    <div class="preview-column">
    <div class="preview-area" bind:this={previewContainer} onscroll={handlePreviewScroll}>
      {#if isEbook}
        <div class="ebook-placeholder">
          <i class="bi bi-phone" style="font-size: 2rem;"></i>
          <p>Ebook preview coming soon</p>
          <p class="ebook-hint">Ebooks use reflowable HTML — no fixed pages to preview.</p>
        </div>
      {:else if rendering}
        <div class="render-loading">
          <div class="loader"></div>
          <p>Compiling pages...</p>
        </div>
      {:else if renderError}
        <div class="render-error">
          <i class="bi bi-exclamation-triangle"></i>
          <p>Rendering failed</p>
          <pre class="error-detail">{renderError}</pre>
          <button class="retry-btn" onclick={compileAndShow}>Retry</button>
        </div>
      {:else if pageCount === 0}
        <div class="render-loading">
          <p>No pages to display</p>
        </div>
      {:else}
        <div class="preview-scroll">
          {#each Array(pageCount) as _, i}
            <div class="preview-page-wrap" id="preview-page-{i}" data-page-index={i}>
              {#if loadedPages.has(i)}
                <img
                  class="preview-page-img"
                  src="http://iwe.localhost/preview/page/{i}.svg?v={compileGeneration}"
                  alt="Page {i + 1}"
                  draggable="false"
                />
              {:else}
                <div class="preview-page-placeholder"
                  style="width: {(activeProfile?.trim_width_in ?? 6) * 72}px; aspect-ratio: {activeProfile?.trim_width_in ?? 6} / {activeProfile?.trim_height_in ?? 9};"></div>
              {/if}
              <div class="page-number">{i + 1}</div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    {#if lastTiming}
      <div class="timing-bar">
        <span title="Total compile time">{lastTiming.total_ms.toFixed(0)}ms</span>
        <span class="timing-sep">|</span>
        <span title="DB load">db:{lastTiming.db_load_ms.toFixed(0)}</span>
        <span title="Y.Doc text extraction">ydoc:{lastTiming.ydoc_extract_ms.toFixed(0)}</span>
        <span title="Typst markup generation">markup:{lastTiming.markup_build_ms.toFixed(0)}</span>
        <span title="Typst compilation">compile:{lastTiming.typst_compile_ms.toFixed(0)}</span>
        <span class="timing-sep">|</span>
        <span>{lastTiming.page_count} pages</span>
      </div>
    {/if}
    </div><!-- end preview-column -->

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
      <!-- Profile selector -->
      <div class="sidebar-section profile-section">
        <div class="profile-row">
          {#if renamingProfileId === activeProfileId}
            <input class="profile-rename-input"
              bind:value={renameValue}
              onblur={commitRename}
              onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renamingProfileId = null; }}
              autofocus />
          {:else}
            <select class="profile-select"
              value={activeProfileId}
              onchange={(e) => switchProfile(Number(e.target.value))}>
              {#each profiles as p}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          {/if}
          <button class="profile-action-btn" title="Rename" onclick={startRenameActive}>
            <i class="bi bi-pencil"></i>
          </button>
          <button class="profile-action-btn" title="Copy settings (excludes target/size)"
            onclick={copyProfileSettings}>
            <i class="bi bi-clipboard"></i>
          </button>
          <button class="profile-action-btn"
            title={formatClipboard
              ? `Paste settings from "${formatClipboard.name}" (target/size unchanged)`
              : 'Nothing to paste — copy a profile first'}
            disabled={!formatClipboard || formatClipboard.id === activeProfileId}
            onclick={pasteProfileSettings}>
            <i class="bi bi-clipboard-check"></i>
          </button>
          <button class="profile-action-btn" title="New profile" onclick={openCreateProfileModal}>
            <i class="bi bi-plus-lg"></i>
          </button>
          <button class="profile-action-btn danger"
            title={profiles.length <= 1 ? 'Cannot delete the only profile' : 'Delete profile'}
            disabled={profiles.length <= 1}
            onclick={() => confirmDeleteProfileId = activeProfileId}>
            <i class="bi bi-trash"></i>
          </button>
        </div>
        {#if confirmDeleteProfileId === activeProfileId}
          <div class="confirm-delete">
            Delete "{activeProfile?.name}"?
            <button class="confirm-yes" onclick={deleteActiveProfile}>Yes</button>
            <button class="confirm-no" onclick={() => confirmDeleteProfileId = null}>No</button>
          </div>
        {/if}
      </div>

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
            <!-- Tag bar: click to arm, then click a page in the list to insert below -->
            <div class="tag-bar">
              {#each ASSIGNABLE_ROLES as role}
                {@const used = usedTags.has(role)}
                <button class="tag-pill"
                  class:used
                  class:armed={armedTag === role}
                  title={used ? `Already used — click × to remove` : (armedTag === role ? 'Click a page to insert below it (Esc to cancel)' : `Add a ${roleLabel(role)} page`)}
                  onclick={() => armTag(role)}>
                  <span>{roleLabel(role)}</span>
                  {#if used}
                    <span class="tag-x" title="Remove this page"
                      onclick={(e) => { e.stopPropagation(); deleteTaggedPage(role); }}>×</span>
                  {/if}
                </button>
              {/each}
            </div>
            {#if armedTag}
              <div class="armed-hint">Click a page to insert <strong>{roleLabel(armedTag)}</strong> below it · <kbd>Esc</kbd> to cancel</div>
            {/if}

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
                <div class="page-list-entry" animate:flip={{ duration: flipDurationMs }}>
                  <div class="page-list-item format-page-item" class:armed-target={armedTag} onclick={() => editingPageId === item.id ? null : handlePageItemClick(item)}>
                    <i class="bi bi-grip-vertical drag-handle"></i>
                    {#if editingPageId === item.id}
                      <input class="page-item-input"
                        bind:value={editingPageTitle}
                        onblur={commitEditPage}
                        onkeydown={(e) => { if (e.key === 'Enter') commitEditPage(); }}
                        onclick={(e) => e.stopPropagation()}
                        autofocus />
                    {:else}
                      <span class="page-item-title">{item.title || roleLabel(item.page_role)}</span>
                    {/if}
                    <span class="page-item-role">{roleLabel(item.page_role)}</span>
                    <button class="page-item-edit" title="Edit content"
                      onclick={(e) => { e.stopPropagation(); openDesignEditor(item); }}>
                      <i class="bi bi-card-text"></i>
                    </button>
                    <button class="page-item-edit" title="Rename"
                      onclick={(e) => { e.stopPropagation(); startEditPage(item); }}>
                      <i class="bi bi-pencil"></i>
                    </button>
                    <button class="page-item-delete" title="Remove page"
                      onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                      <i class="bi bi-x"></i>
                    </button>
                  </div>
                  <div class="profile-pills">
                    {#each profiles as p}
                      <button class="profile-pill"
                        class:included={isPageIncludedIn(item.id, p.id)}
                        title={isPageIncludedIn(item.id, p.id) ? `Included in ${p.name}` : `Excluded from ${p.name}`}
                        onclick={(e) => { e.stopPropagation(); togglePageInProfile(item.id, p.id); }}>
                        {p.name}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>

            <!-- Chapters (locked, non-draggable) -->
            <div class="page-group-label">Chapters</div>
            {#each chapters as ch (ch.id)}
              <div class="page-list-item chapter-item"
                onclick={() => handleChapterItemClick(ch)}>
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
                <div class="page-list-entry" animate:flip={{ duration: flipDurationMs }}>
                  <div class="page-list-item format-page-item" class:armed-target={armedTag} onclick={() => editingPageId === item.id ? null : handlePageItemClick(item)}>
                    <i class="bi bi-grip-vertical drag-handle"></i>
                    {#if editingPageId === item.id}
                      <input class="page-item-input"
                        bind:value={editingPageTitle}
                        onblur={commitEditPage}
                        onkeydown={(e) => { if (e.key === 'Enter') commitEditPage(); }}
                        onclick={(e) => e.stopPropagation()}
                        autofocus />
                    {:else}
                      <span class="page-item-title">{item.title || roleLabel(item.page_role)}</span>
                    {/if}
                    <span class="page-item-role">{roleLabel(item.page_role)}</span>
                    <button class="page-item-edit" title="Edit content"
                      onclick={(e) => { e.stopPropagation(); openDesignEditor(item); }}>
                      <i class="bi bi-card-text"></i>
                    </button>
                    <button class="page-item-edit" title="Rename"
                      onclick={(e) => { e.stopPropagation(); startEditPage(item); }}>
                      <i class="bi bi-pencil"></i>
                    </button>
                    <button class="page-item-delete" title="Remove page"
                      onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                      <i class="bi bi-x"></i>
                    </button>
                  </div>
                  <div class="profile-pills">
                    {#each profiles as p}
                      <button class="profile-pill"
                        class:included={isPageIncludedIn(item.id, p.id)}
                        title={isPageIncludedIn(item.id, p.id) ? `Included in ${p.name}` : `Excluded from ${p.name}`}
                        onclick={(e) => { e.stopPropagation(); togglePageInProfile(item.id, p.id); }}>
                        {p.name}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {:else if sidebarMode === 'themes'}
          <div class="mode-panel">
            <p class="shell-placeholder">Theme presets will appear here.</p>
          </div>
        {:else if sidebarMode === 'custom'}
          <div class="mode-panel">
            <!-- Sub-tab selector dropdown -->
            <div class="custom-selector-wrap">
              <button class="custom-selector-btn"
                onclick={() => customSelectorOpen = !customSelectorOpen}>
                <i class="bi {activeCustomTab.icon}"></i>
                <span class="custom-selector-label">{activeCustomTab.label}</span>
                <i class="bi bi-chevron-down custom-selector-chevron" class:open={customSelectorOpen}></i>
              </button>
              {#if customSelectorOpen}
                <div class="custom-selector-backdrop"
                  onclick={() => customSelectorOpen = false}
                  role="button" tabindex="-1" onkeydown={() => {}}></div>
                <div class="custom-selector-dropdown">
                  {#each CUSTOM_TABS as tab (tab.id)}
                    <button class="custom-option"
                      class:active={customTab === tab.id}
                      onclick={() => selectCustomTab(tab.id)}>
                      <i class="bi {tab.icon}"></i>
                      <span>{tab.label}</span>
                      {#if customTab === tab.id}
                        <i class="bi bi-check2 custom-option-check"></i>
                      {/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>

            <!-- Active sub-tab component -->
            {#if customTab === 'chapter-headings'}
              <ChapterHeadings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'paragraph'}
              <ParagraphSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'headings'}
              <HeadingsSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'breaks'}
              <BreaksSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'print-layout'}
              <PrintLayoutSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'typography'}
              <TypographySettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'header-footer'}
              <HeaderFooterSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'trim'}
              <TrimSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {/if}
          </div>
        {:else if sidebarMode === 'export'}
          <div class="mode-panel">
            <div class="export-panel">
              <h4 class="export-title">Export</h4>

              {#if activeProfile}
                <div class="export-info">
                  <div class="export-info-row">
                    <span class="export-info-label">Profile</span>
                    <span class="export-info-value">{activeProfile.name}</span>
                  </div>
                  <div class="export-info-row">
                    <span class="export-info-label">Trim</span>
                    <span class="export-info-value">{activeProfile.trim_width_in}″ × {activeProfile.trim_height_in}″</span>
                  </div>
                  <div class="export-info-row">
                    <span class="export-info-label">Pages</span>
                    <span class="export-info-value">{pageCount || '—'}</span>
                  </div>
                </div>
              {/if}

              <button class="export-btn-main" onclick={handleExportPdf}
                disabled={exporting || pageCount === 0}>
                {#if exporting}
                  <span class="export-spinner"></span> Exporting...
                {:else}
                  <i class="bi bi-file-earmark-pdf"></i> Export PDF
                {/if}
              </button>

              {#if exportError}
                <div class="export-error">
                  <i class="bi bi-exclamation-triangle"></i>
                  {exportError}
                </div>
              {/if}

              <p class="export-hint">
                Exports the current profile as a print-ready PDF with all fonts embedded.
                Make sure the preview looks correct before exporting.
              </p>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if designingPage}
    <PageContentEditor
      page={designingPage}
      profile={activeProfile}
      onsave={saveDesignedPage}
      oncancel={cancelDesignedPage} />
  {/if}

  {#if showCreateProfileModal}
    <div class="modal-backdrop" onclick={() => showCreateProfileModal = false}>
      <div class="modal-card" onclick={(e) => e.stopPropagation()}>
        <h3>New Profile</h3>
        <label class="modal-label">Name</label>
        <input class="modal-input" type="text" bind:value={newProfileName}
          placeholder="e.g. 5×8 Mass Market" autofocus
          onkeydown={(e) => { if (e.key === 'Enter') createProfile(); }} />

        <label class="modal-label">Duplicate from existing profile</label>
        <select class="modal-input" bind:value={newProfileDuplicateFrom}>
          <option value={null}>— Start blank —</option>
          {#each profiles as p}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>

        {#if !newProfileDuplicateFrom}
          <label class="modal-label">Target type</label>
          <div class="target-type-toggle">
            <button class="tt-btn" class:active={newProfileTargetType === 'print'}
              onclick={() => newProfileTargetType = 'print'}>Print</button>
            <button class="tt-btn" class:active={newProfileTargetType === 'ebook'}
              onclick={() => newProfileTargetType = 'ebook'}>Ebook</button>
          </div>
        {/if}

        <div class="modal-actions">
          <button class="modal-btn" onclick={() => showCreateProfileModal = false}>Cancel</button>
          <button class="modal-btn primary" onclick={createProfile} disabled={!newProfileName.trim()}>Create</button>
        </div>
      </div>
    </div>
  {/if}
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

  .preview-column {
    flex: 1; display: flex; flex-direction: column; min-width: 0; overflow: hidden;
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
    padding-bottom: 2rem;
  }
  .preview-page-wrap {
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
  }
  .preview-page-img {
    display: block;
    max-width: 100%;
    height: auto;
    box-shadow: 0 2px 12px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.06);
    background: #fff;
  }
  .preview-page-placeholder {
    background: #f5f3f0;
    border: 1px dashed rgba(0,0,0,0.12);
    box-shadow: 0 2px 12px rgba(0,0,0,0.06);
  }
  .page-number {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-align: center;
  }

  /* Ebook placeholder */
  .ebook-placeholder {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 0.5rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .ebook-placeholder p { margin: 0; }
  .ebook-hint { font-size: 0.78rem; font-style: italic; }

  /* Render states */
  .render-loading {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 0.8rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .render-error {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 0.6rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .render-error i { font-size: 1.5rem; color: #d97706; }
  .error-detail {
    font-size: 0.72rem; max-width: 500px; overflow-x: auto;
    background: var(--iwe-bg); padding: 0.5rem 0.8rem;
    border-radius: var(--iwe-radius-sm); border: 1px solid var(--iwe-border);
    white-space: pre-wrap; word-break: break-word;
  }
  .retry-btn {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.35rem 0.9rem; border: 1px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm); background: none;
    color: var(--iwe-accent); cursor: pointer; transition: all 150ms;
  }
  .retry-btn:hover { background: var(--iwe-accent); color: #fff; }

  /* Timing bar */
  .timing-bar {
    flex-shrink: 0; display: flex; align-items: center; gap: 0.5rem;
    padding: 0.2rem 0.8rem; height: 24px;
    background: var(--iwe-bg-warm); border-top: 1px solid var(--iwe-border);
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); white-space: nowrap; overflow-x: auto;
  }
  .timing-sep { color: var(--iwe-border); }

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

  /* Profile selector */
  .profile-section { padding: 0.6rem 0.9rem; }
  .profile-row { display: flex; align-items: center; gap: 0.3rem; }
  .profile-select {
    flex: 1; padding: 0.3rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    min-width: 0;
  }
  .profile-select:focus { outline: none; border-color: var(--iwe-accent); }
  .profile-rename-input {
    flex: 1; padding: 0.3rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-accent); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); min-width: 0;
  }
  .profile-rename-input:focus { outline: none; }
  .profile-action-btn {
    border: 1px solid transparent; background: none;
    color: var(--iwe-text-muted); cursor: pointer;
    padding: 0.3rem 0.4rem; font-size: 0.85rem; line-height: 1;
    border-radius: var(--iwe-radius-sm); transition: all 120ms;
  }
  .profile-action-btn:hover:not(:disabled) {
    color: var(--iwe-accent); border-color: var(--iwe-accent);
  }
  .profile-action-btn:disabled { opacity: 0.3; cursor: not-allowed; }
  .profile-action-btn.danger:hover:not(:disabled) {
    color: #c0392b; border-color: #c0392b;
  }
  .confirm-delete {
    margin-top: 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text); display: flex; align-items: center; gap: 0.4rem;
  }
  .confirm-yes, .confirm-no {
    border: 1px solid var(--iwe-border); background: none;
    padding: 0.2rem 0.6rem; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    border-radius: var(--iwe-radius-sm);
  }
  .confirm-yes { color: #c0392b; border-color: #c0392b; }
  .confirm-yes:hover { background: #c0392b; color: #fff; }
  .confirm-no:hover { background: var(--iwe-bg-hover); }

  /* Modal */
  .modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.4);
    display: flex; align-items: center; justify-content: center;
    z-index: 1000;
  }
  .modal-card {
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); padding: 1.4rem;
    min-width: 360px; max-width: 90vw;
    box-shadow: 0 20px 60px rgba(0,0,0,0.25);
    font-family: var(--iwe-font-ui);
  }
  .modal-card h3 {
    margin: 0 0 1rem 0;
    font-family: var(--iwe-font-prose); font-weight: 400;
    color: var(--iwe-text);
  }
  .modal-label {
    display: block; font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin: 0.8rem 0 0.3rem 0;
  }
  .modal-input {
    width: 100%; padding: 0.45rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .modal-input:focus { outline: none; border-color: var(--iwe-accent); }
  .target-type-toggle {
    display: flex; gap: 0.4rem;
  }
  .tt-btn {
    flex: 1; padding: 0.45rem 0;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text-muted); cursor: pointer;
    transition: all 120ms;
  }
  .tt-btn.active {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06); font-weight: 500;
  }
  .modal-actions {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    margin-top: 1.4rem;
  }
  .modal-btn {
    padding: 0.45rem 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 120ms;
  }
  .modal-btn:hover:not(:disabled) { background: var(--iwe-bg-hover); }
  .modal-btn.primary {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .modal-btn.primary:hover:not(:disabled) {
    background: #245a4f; border-color: #245a4f;
  }
  .modal-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  /* Profile pills (per-page inclusion) */
  .page-list-entry {
    margin-bottom: 0.3rem;
  }
  .profile-pills {
    display: flex; flex-wrap: wrap; gap: 4px;
    padding: 0.25rem 0.55rem 0.4rem 1.85rem;
  }
  .profile-pill {
    border: 1px solid var(--iwe-border); background: none;
    color: var(--iwe-text-muted);
    padding: 2px 8px; border-radius: 10px;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    cursor: pointer; transition: all 100ms;
    max-width: 100%; overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap;
  }
  .profile-pill.included {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .profile-pill:not(.included):hover {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
  }
  .profile-pill.included:hover { opacity: 0.85; }
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


  /* Custom mode sub-tab selector */
  .custom-selector-wrap {
    position: relative;
    margin-bottom: 0.6rem;
  }
  .custom-selector-btn {
    width: 100%;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
  }
  .custom-selector-btn:hover { border-color: var(--iwe-accent); }
  .custom-selector-label { flex: 1; text-align: left; }
  .custom-selector-chevron {
    font-size: 0.7rem; color: var(--iwe-text-muted);
    transition: transform 150ms;
  }
  .custom-selector-chevron.open { transform: rotate(180deg); }
  .custom-selector-backdrop {
    position: fixed; inset: 0; z-index: 5;
    background: transparent; cursor: default;
  }
  .custom-selector-dropdown {
    position: absolute; top: calc(100% + 4px); left: 0; right: 0;
    z-index: 10;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    box-shadow: 0 8px 24px rgba(0,0,0,0.12);
    padding: 4px;
    max-height: 60vh; overflow-y: auto;
  }
  .custom-option {
    width: 100%;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: none; background: none; color: var(--iwe-text); cursor: pointer;
    border-radius: var(--iwe-radius-sm); text-align: left;
    transition: background 100ms;
  }
  .custom-option:hover { background: var(--iwe-bg-hover); }
  .custom-option.active { color: var(--iwe-accent); font-weight: 500; }
  .custom-option i:first-child { width: 16px; text-align: center; }
  .custom-option-check {
    margin-left: auto; color: var(--iwe-accent);
  }

  /* Export panel */
  .export-panel { padding: 0.4rem 0; }
  .export-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 1rem 0; color: var(--iwe-text);
  }
  .export-info {
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.6rem 0.8rem;
    margin-bottom: 1rem;
  }
  .export-info-row {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.25rem 0;
  }
  .export-info-row + .export-info-row { border-top: 1px solid var(--iwe-border); }
  .export-info-label {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
  }
  .export-info-value {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text);
  }
  .export-btn-main {
    width: 100%;
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    padding: 0.75rem 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.95rem; font-weight: 500;
    background: var(--iwe-accent); border: 1px solid var(--iwe-accent);
    color: #fff; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .export-btn-main:hover:not(:disabled) { background: #245a4f; }
  .export-btn-main:disabled { opacity: 0.5; cursor: not-allowed; }
  .export-btn-main i { font-size: 1.1rem; }
  .export-spinner {
    width: 16px; height: 16px;
    border: 2px solid rgba(255,255,255,0.3); border-top-color: #fff;
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  .export-error {
    margin-top: 0.7rem;
    padding: 0.5rem 0.7rem;
    background: rgba(192, 57, 43, 0.08);
    border: 1px solid rgba(192, 57, 43, 0.2);
    border-radius: var(--iwe-radius-sm);
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: #c0392b;
    display: flex; align-items: flex-start; gap: 0.4rem;
  }
  .export-hint {
    margin-top: 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); line-height: 1.5;
  }

  /* Tag bar */
  .tag-bar {
    display: flex; flex-wrap: wrap; gap: 4px;
    padding: 0.5rem 0 0.5rem 0;
    border-bottom: 1px solid var(--iwe-border);
    margin-bottom: 0.4rem;
  }
  .tag-pill {
    display: inline-flex; align-items: center; gap: 3px;
    border: 1px solid var(--iwe-border); background: var(--iwe-bg);
    color: var(--iwe-text);
    padding: 3px 10px; border-radius: 11px;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    cursor: pointer; transition: all 100ms;
  }
  .tag-pill:hover:not(.used) {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
  }
  .tag-pill.armed {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
    box-shadow: 0 0 0 2px rgba(45, 106, 94, 0.2);
  }
  .tag-pill.used {
    background: var(--iwe-bg-hover); color: var(--iwe-text-muted);
    cursor: default; opacity: 0.7;
  }
  .tag-x {
    display: inline-flex; align-items: center; justify-content: center;
    width: 14px; height: 14px; border-radius: 50%;
    color: var(--iwe-text-muted); font-size: 0.85rem; line-height: 1;
    cursor: pointer; transition: all 100ms;
  }
  .tag-x:hover {
    background: #c0392b; color: #fff;
  }
  .armed-hint {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); padding: 0.3rem 0.4rem;
    background: rgba(45, 106, 94, 0.06);
    border-left: 2px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm);
    margin-bottom: 0.5rem;
  }
  .armed-hint kbd {
    font-family: monospace; font-size: 0.65rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    padding: 0 4px; border-radius: 3px;
  }

  /* Page list — when a tag is armed, page items become click targets */
  .page-list-item.armed-target {
    cursor: copy;
  }
  .page-list-item.armed-target:hover {
    background: rgba(45, 106, 94, 0.12);
    box-shadow: inset 0 -2px 0 var(--iwe-accent);
  }

  /* Page list */
  .page-group-label {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    padding: 0.6rem 0 0.3rem 0;
    display: flex; align-items: center; justify-content: space-between;
  }
  .add-page-btn {
    border: none; background: none; color: var(--iwe-accent);
    cursor: pointer; padding: 0.1rem 0.35rem; font-size: 1.1rem;
    line-height: 1; border-radius: var(--iwe-radius-sm);
    transition: background 100ms;
  }
  .add-page-btn:hover { background: var(--iwe-bg-hover); }

  .dnd-zone { min-height: 4px; }

  .page-list-item {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.55rem; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: background 100ms;
    font-family: var(--iwe-font-ui); font-size: 0.92rem;
  }
  .page-list-item:hover { background: var(--iwe-bg-hover); }

  .format-page-item .drag-handle {
    color: var(--iwe-text-muted); font-size: 1.05rem;
    cursor: grab; opacity: 0.5;
  }
  .format-page-item:hover .drag-handle { opacity: 1; }

  .page-item-title {
    flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--iwe-text);
  }
  .page-item-role {
    font-size: 0.72rem; color: var(--iwe-text-muted);
    white-space: nowrap;
  }
  .page-item-edit, .page-item-delete {
    border: none; background: none; color: var(--iwe-text-muted);
    cursor: pointer; padding: 0.15rem 0.3rem; font-size: 1rem;
    opacity: 0; transition: opacity 100ms, color 100ms;
  }
  .page-list-item:hover .page-item-edit,
  .page-list-item:hover .page-item-delete { opacity: 0.75; }
  .page-item-edit:hover { color: var(--iwe-accent); opacity: 1 !important; }
  .page-item-delete:hover { color: #c0392b; opacity: 1 !important; }
  .page-item-input {
    flex: 1; min-width: 0;
    border: 1px solid var(--iwe-accent); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    font-family: var(--iwe-font-ui); font-size: 0.92rem;
    padding: 0.2rem 0.4rem;
  }
  .page-item-input:focus { outline: none; }

  /* Chapter items are visually distinct */
  .chapter-item {
    padding-left: 1.6rem;
    opacity: 0.75;
    cursor: pointer;
  }
  .chapter-item .page-item-title {
    font-style: italic;
  }

</style>
