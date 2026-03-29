<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { getProjectsDir, getSettings, saveSettings, openProject, getChapters, getChapter, addChapter, updateChapterContent, renameChapter, deleteChapter, getEntities, createEntity, updateEntity, deleteEntity, setEntityVisible, addAlias, removeAlias, scanAllChapters, getNavHistory, pushNavEntry, truncateNavAfter, addEntityNote, logWritingActivity } from '$lib/db.js';
  import ChapterNav from '$lib/components/ChapterNav.svelte';
  import Editor from '$lib/components/Editor.svelte';
  import EntityPanel from '$lib/components/EntityPanel.svelte';
  import Toasts from '$lib/components/Toasts.svelte';
  import { buildTextMap } from '$lib/entityHighlight.js';
  import { exportDocx, exportTxt, exportHtml, exportPdf } from '$lib/export.js';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import SearchPanel from '$lib/components/SearchPanel.svelte';
  import AnalysisPanel from '$lib/components/AnalysisPanel.svelte';
  import { addToast } from '$lib/toast.js';

  let { data } = $props();

  let projectTitle = $state('');
  let chapters = $state([]);
  let openTabs = $state([]);
  let activeTabId = $state(null);
  let activeChapter = $state(null);
  let saveTimer = null;
  let lastKnownWordCounts = {}; // chapterId -> word count, for tracking deltas
  let entities = $state([]);
  let selectedText = $state('');
  let lastSelectedText = ''; // persists even after selection clears (for pinning)
  let editorRef;

  // Derived from entity.visible — the Set that drives highlighting
  let viewedEntityIds = $derived(new Set(entities.filter(e => e.visible).map(e => e.id)));

  // Navigation history (persisted to DB)
  let navHistory = $state([]); // NavEntry objects from DB
  let navIndex = $state(-1);
  let navigating = false;
  let checkpointTimer = null;
  let lastCheckpointChapter = null;
  let lastCheckpointScroll = 0;

  function captureLocation() {
    if (!activeChapter) return null;
    const ed = editorRef?.getEditor();
    const scrollEl = document.querySelector('.editor-scroll');
    return {
      chapterId: activeChapter.id,
      scrollTop: scrollEl?.scrollTop || 0,
      cursorPos: ed?.state.selection.from || 0,
    };
  }

  function isSignificantMove(loc) {
    if (!loc) return false;
    if (navHistory.length === 0) return true;
    const last = navHistory[navIndex >= 0 ? navIndex : navHistory.length - 1];
    if (!last) return true;
    // Different chapter = always significant
    if (last.chapter_id !== loc.chapterId) return true;
    // Same chapter, big scroll distance
    if (Math.abs(last.scroll_top - loc.scrollTop) > 400) return true;
    return false;
  }

  async function pushNavCheckpoint(force = false) {
    if (navigating) return;
    const loc = captureLocation();
    if (!loc) return;
    if (!force && !isSignificantMove(loc)) return;

    // If we're in the middle of history, truncate forward entries
    if (navIndex >= 0 && navIndex < navHistory.length - 1) {
      const currentEntry = navHistory[navIndex];
      if (currentEntry) {
        await truncateNavAfter(currentEntry.id);
      }
    }

    const id = await pushNavEntry(loc.chapterId, loc.scrollTop, loc.cursorPos);
    navHistory = await getNavHistory();
    navIndex = navHistory.length - 1;
  }

  // Auto-checkpoint: if stationary for 10 seconds at a significant distance
  function resetCheckpointTimer() {
    clearTimeout(checkpointTimer);
    checkpointTimer = setTimeout(() => {
      pushNavCheckpoint(false);
    }, 10000);
  }

  async function navBack() {
    if (navIndex <= 0) return;
    // Save current position if we're at the tip
    if (navIndex === navHistory.length - 1) {
      await pushNavCheckpoint(true);
      navIndex = navHistory.length - 2;
    } else {
      navIndex--;
    }
    await navGoTo(navHistory[navIndex]);
  }

  async function navForward() {
    if (navIndex >= navHistory.length - 1) return;
    navIndex++;
    await navGoTo(navHistory[navIndex]);
  }

  async function navGoTo(entry) {
    navigating = true;
    await selectChapter(entry.chapter_id);
    setTimeout(() => {
      const scrollEl = document.querySelector('.editor-scroll');
      if (scrollEl) scrollEl.scrollTop = entry.scroll_top;
      const ed = editorRef?.getEditor();
      if (ed && entry.cursor_pos) {
        try {
          ed.chain().focus().setTextSelection(entry.cursor_pos).run();
        } catch { /* pos may be out of range */ }
      }
      navigating = false;
    }, 200);
  }

  let canGoBack = $derived(navIndex > 0);
  let canGoForward = $derived(navIndex < navHistory.length - 1);

  // Panel widths (resizable, persisted)
  let leftWidth = $state(230);
  let rightWidth = $state(260);
  let dragging = $state(null); // 'left' | 'right' | null
  let saveWidthTimer = null;

  const LEFT_MIN = 140;
  const LEFT_MAX = 450;
  const RIGHT_MIN = 160;
  const RIGHT_MAX = 1000;

  onMount(async () => {
    const dir = await getProjectsDir();
    if (!dir) { goto('/'); return; }

    // Load saved panel widths
    const settings = await getSettings();
    if (settings.leftPanelWidth) leftWidth = settings.leftPanelWidth;
    if (settings.rightPanelWidth) rightWidth = settings.rightPanelWidth;

    const filepath = `${dir}/${data.filename}`;
    await openProject(filepath);
    projectTitle = data.filename.replace('.iwe', '');
    chapters = await getChapters();
    entities = await getEntities();
    refreshChapterCounts();

    // Initialize word count tracking
    for (const ch of chapters) {
      lastKnownWordCounts[ch.id] = wordCount(ch.content);
    }

    // Load nav history
    navHistory = await getNavHistory();
    navIndex = navHistory.length > 0 ? navHistory.length - 1 : -1;

    if (chapters.length > 0) {
      await selectChapter(chapters[0].id);
    }

    // Set up scroll listener for auto-checkpointing
    setTimeout(() => {
      const scrollEl = document.querySelector('.editor-scroll');
      if (scrollEl) {
        scrollEl.addEventListener('scroll', () => resetCheckpointTimer());
      }
    }, 500);
  });

  let chapterCounts = $state({});

  async function refreshChapterCounts() {
    try {
      const counts = await scanAllChapters();
      const map = {};
      for (const c of counts) {
        if (c.total > 0) map[c.chapter_id] = c.total;
      }
      chapterCounts = map;
    } catch {
      // Scanner not ready yet
    }
  }

  async function toggleViewEntity(entityId) {
    const entity = entities.find(e => e.id === entityId);
    if (!entity) return;
    await setEntityVisible(entityId, !entity.visible);
    entities = await getEntities();
    triggerRescan();
  }

  async function handleEntityClick(entityId, entityName, isCtrl) {
    if (isCtrl && lastSelectedText && lastSelectedText.trim()) {
      // Ctrl+click with selection → pin text to entity
      await addEntityNote(entityId, activeChapter?.id || null, lastSelectedText.trim());
      addToast(`Pinned to ${entityName}`, 'success');
      lastSelectedText = '';
      selectedText = '';
      editorRef?.clearSelection();
    }
    // Always open the entity view pane
    focusEntityId = entityId;
    focusTrigger++;
  }

  let focusEntityId = $state(null);
  let focusTrigger = $state(0);
  let rightPanelTab = $state('entities'); // 'entities' | 'search' | 'analysis'

  let pendingEntityName = $state(null);

  let showExportMenu = $state(false);

  async function handleExport(format) {
    showExportMenu = false;
    try {
      let path;
      if (format === 'docx') path = await exportDocx(chapters, projectTitle);
      else if (format === 'txt') path = await exportTxt(chapters, projectTitle);
      else if (format === 'html') path = await exportHtml(chapters, projectTitle);
      else if (format === 'pdf-a4') path = await exportPdf(chapters, projectTitle, 'a4');
      else if (format === 'pdf-book') path = await exportPdf(chapters, projectTitle, 'book');
      if (path) addToast(`Exported to ${path.split(/[/\\]/).pop()}`, 'success');
    } catch (e) {
      console.error('Export failed:', e);
      addToast('Export failed: ' + e, 'error');
    }
  }

  function launchWritingStats() {
    try {
      new WebviewWindow('stats-' + Date.now(), {
        url: '/stats',
        title: 'Writing Stats',
        width: 1100,
        height: 800,
        resizable: true,
      });
    } catch (e) { console.error('Failed to open stats:', e); }
  }

  function handleQuickAdd(word) {
    pendingEntityName = word;
  }

  /**
   * Universal jump-to-position. Used by EVERYTHING.
   *
   * @param chapterId - which chapter to open
   * @param searchText - the word/phrase to highlight
   * @param anchorOrPosition - either a char_position (number) from Rust, or an anchor string (legacy)
   */
  async function handleGoToChapter(chapterId, searchText, anchorOrPosition) {
    await pushNavCheckpoint(true);
    await selectChapter(chapterId);
    setTimeout(() => {
      const ed = editorRef?.getEditor();
      if (!ed) return;

      let pmFrom = null;
      let pmTo = null;

      // If anchorOrPosition is a number, use it as char_position (from Rust scanner)
      if (typeof anchorOrPosition === 'number') {
        const { text, posMap } = buildTextMap(ed.state.doc);
        const charPos = anchorOrPosition;
        const matchLen = searchText ? searchText.length : 1;

        if (charPos < posMap.length && posMap[charPos] >= 0) {
          pmFrom = posMap[charPos];
          const endCharPos = charPos + matchLen - 1;
          pmTo = endCharPos < posMap.length && posMap[endCharPos] >= 0
            ? posMap[endCharPos] + 1
            : pmFrom + matchLen;
        }
      }

      // Fallback: word-sequence search (for anchors and legacy callers)
      if (pmFrom === null) {
        const docWords = [];
        ed.state.doc.descendants((node, pos) => {
          if (!node.isText) return;
          const re = /[a-zA-Z0-9\u00C0-\u024F']+/g;
          let m;
          while ((m = re.exec(node.text)) !== null) {
            docWords.push({ word: m[0].toLowerCase(), pos: pos + m.index, len: m[0].length });
          }
        });

        function toWords(text) {
          if (!text) return [];
          return text.replace(/[^a-zA-Z0-9\u00C0-\u024F'\s]/g, ' ')
            .trim().split(/\s+/).filter(w => w.length > 0).map(w => w.toLowerCase());
        }

        function findSequence(words) {
          if (words.length === 0) return -1;
          for (let i = 0; i <= docWords.length - words.length; i++) {
            let match = true;
            for (let j = 0; j < words.length; j++) {
              if (docWords[i + j].word !== words[j]) { match = false; break; }
            }
            if (match) return i;
          }
          return -1;
        }

        // Try anchor words, then searchText words, then first word
        const attempts = [
          anchorOrPosition ? toWords(String(anchorOrPosition)) : [],
          searchText ? toWords(searchText) : [],
        ].filter(a => a.length > 0);

        for (const words of attempts) {
          const idx = findSequence(words);
          if (idx >= 0) {
            pmFrom = docWords[idx].pos;
            const last = docWords[idx + words.length - 1];
            pmTo = last.pos + last.len;
            break;
          }
        }

        // Last resort: find first word
        if (pmFrom === null && searchText) {
          const firstWord = toWords(searchText)[0];
          if (firstWord) {
            const idx = docWords.findIndex(dw => dw.word === firstWord);
            if (idx >= 0) {
              pmFrom = docWords[idx].pos;
              pmTo = pmFrom + docWords[idx].len;
            }
          }
        }
      }

      if (pmFrom === null) return;

      ed.chain().focus().setTextSelection({ from: pmFrom, to: pmTo }).run();

      // Delay scroll to after decoration transactions settle
      setTimeout(() => {
        const coords = ed.view.coordsAtPos(pmFrom);
        const scrollEl = document.querySelector('.editor-scroll');
        if (scrollEl) {
          const scrollRect = scrollEl.getBoundingClientRect();
          const targetScroll = scrollEl.scrollTop + (coords.top - scrollRect.top) - (scrollRect.height / 2);
          const maxScroll = scrollEl.scrollHeight - scrollEl.clientHeight;
          scrollEl.scrollTo({ top: Math.max(0, Math.min(targetScroll, maxScroll)), behavior: 'smooth' });
        }
        flashJumpHighlight(ed, pmFrom, pmTo);
      }, 100);
    }, 250);
  }

  function flashJumpHighlight(ed, from, to) {
    setTimeout(() => {
      try {
        const scrollEl = document.querySelector('.editor-scroll');
        if (!scrollEl) return;
        const scrollRect = scrollEl.getBoundingClientRect();

        // Build a DOM range from the PM positions
        const domFrom = ed.view.domAtPos(from);
        const domTo = ed.view.domAtPos(to);
        if (!domFrom?.node || !domTo?.node) return;

        const range = document.createRange();
        range.setStart(domFrom.node, domFrom.offset);
        range.setEnd(domTo.node, domTo.offset);

        // getClientRects gives one rect per visual line — handles line breaks
        const rects = range.getClientRects();
        if (rects.length === 0) return;

        const container = document.createElement('div');
        container.className = 'jump-flash-container';

        for (const rect of rects) {
          if (rect.width < 1) continue; // skip zero-width rects
          const box = document.createElement('div');
          box.className = 'jump-flash-box';
          box.style.top = (rect.top - scrollRect.top + scrollEl.scrollTop - 2) + 'px';
          box.style.left = (rect.left - scrollRect.left - 3) + 'px';
          box.style.width = (rect.width + 6) + 'px';
          box.style.height = (rect.height + 4) + 'px';
          container.appendChild(box);
        }

        scrollEl.appendChild(container);
        setTimeout(() => container.remove(), 7000);
      } catch { /* positioning can fail, that's ok */ }
    }, 100);
  }

  function triggerRescan() {
    refreshChapterCounts();
    // Pass current visible set directly
    const visibleIds = new Set(entities.filter(e => e.visible).map(e => e.id));
    editorRef?.rescan(visibleIds);
  }

  async function handleCreateEntity(name, type, description, color) {
    await createEntity(name, type, description, color);
    entities = await getEntities();
    triggerRescan();
  }

  async function handleUpdateEntity(id, name, type, description, color) {
    await updateEntity(id, name, type, description, color);
    entities = await getEntities();
    triggerRescan();
  }

  async function handleDeleteEntity(id) {
    await deleteEntity(id);
    entities = await getEntities();
    triggerRescan();
  }

  async function handleAddAlias(entityId, alias) {
    await addAlias(entityId, alias);
    entities = await getEntities();
    triggerRescan();
  }

  async function handleRemoveAlias(entityId, alias) {
    await removeAlias(entityId, alias);
    entities = await getEntities();
    triggerRescan();
  }

  function persistWidths() {
    clearTimeout(saveWidthTimer);
    saveWidthTimer = setTimeout(async () => {
      const settings = await getSettings();
      settings.leftPanelWidth = leftWidth;
      settings.rightPanelWidth = rightWidth;
      await saveSettings(settings);
    }, 300);
  }

  function startDrag(side) {
    dragging = side;

    const onMove = (e) => {
      if (!dragging) return;
      if (dragging === 'left') {
        const newWidth = Math.min(LEFT_MAX, Math.max(LEFT_MIN, e.clientX));
        leftWidth = newWidth;
      } else {
        const newWidth = Math.min(RIGHT_MAX, Math.max(RIGHT_MIN, window.innerWidth - e.clientX));
        rightWidth = newWidth;
      }
    };

    const onUp = () => {
      dragging = null;
      persistWidths();
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

  async function selectChapter(id) {
    // Save current position before navigating away
    if (activeChapter && activeChapter.id !== id && !navigating) {
      await pushNavCheckpoint(true);
    }
    if (!openTabs.find(t => t.id === id)) {
      const ch = await getChapter(id);
      if (ch) openTabs = [...openTabs, ch];
    }
    activeTabId = id;
    activeChapter = await getChapter(id);
  }

  function closeTab(id) {
    openTabs = openTabs.filter(t => t.id !== id);
    if (activeTabId === id) {
      activeTabId = openTabs.length > 0 ? openTabs[openTabs.length - 1].id : null;
      if (activeTabId) {
        getChapter(activeTabId).then(ch => activeChapter = ch);
      } else {
        activeChapter = null;
      }
    }
  }

  async function handleAddChapter() {
    const num = chapters.length + 1;
    const id = await addChapter(`Chapter ${num}`);
    chapters = await getChapters();
    await selectChapter(id);
  }

  async function handleRenameChapter(id, newTitle) {
    await renameChapter(id, newTitle);
    chapters = await getChapters();
    openTabs = openTabs.map(t => t.id === id ? { ...t, title: newTitle } : t);
    if (activeChapter?.id === id) {
      activeChapter = { ...activeChapter, title: newTitle };
    }
  }

  async function handleDeleteChapter(id) {
    await deleteChapter(id);
    closeTab(id);
    chapters = await getChapters();
  }

  function handleContentChange(content) {
    if (!activeChapter) return;
    activeChapter = { ...activeChapter, content };
    openTabs = openTabs.map(t => t.id === activeChapter.id ? { ...t, content } : t);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      updateChapterContent(activeChapter.id, content);
      refreshChapterCounts();

      // Track writing activity
      const currentWords = wordCount(content);
      const lastWords = lastKnownWordCounts[activeChapter.id] || 0;
      const delta = currentWords - lastWords;
      lastKnownWordCounts[activeChapter.id] = currentWords;

      if (delta !== 0) {
        const manuscriptTotal = chapters.reduce((sum, ch) => {
          if (ch.id === activeChapter.id) return sum + currentWords;
          return sum + wordCount(ch.content);
        }, 0);
        console.log(`[stats] logging: chapter=${activeChapter.id} words=${currentWords} manuscript=${manuscriptTotal} delta=${delta}`);
        logWritingActivity(activeChapter.id, currentWords, manuscriptTotal, delta).catch(e => console.error('[stats] log failed:', e));
      }
    }, 500);
  }

  function wordCount(text) {
    if (!text || !text.trim()) return 0;
    const plain = text.replace(/<[^>]*>/g, ' ');
    const trimmed = plain.trim();
    if (!trimmed) return 0;
    return trimmed.split(/\s+/).length;
  }

  let totalWords = $derived(chapters.reduce((sum, ch) => sum + wordCount(ch.content), 0));
  let currentWords = $derived(activeChapter ? wordCount(activeChapter.content) : 0);
</script>

<div class="workspace">
  <!-- Toolbar -->
  <div class="toolbar">
    <a href="/" class="toolbar-back" title="Back to manuscripts">&larr;</a>
    <span class="toolbar-sep"></span>
    <span class="toolbar-title">{projectTitle}</span>
    <span class="toolbar-spacer"></span>
    <div class="export-wrap">
      <button class="stats-btn" onclick={() => showExportMenu = !showExportMenu} title="Export manuscript">
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
    <button class="stats-btn" onclick={launchWritingStats} title="Writing Stats">
      <i class="bi bi-graph-up"></i> Stats
    </button>
    <span class="toolbar-spacer"></span>
    <button class="nav-btn" disabled={!canGoBack} onclick={navBack} title="Go back">
      <i class="bi bi-chevron-left"></i>
    </button>
    <button class="nav-btn" disabled={!canGoForward} onclick={navForward} title="Go forward">
      <i class="bi bi-chevron-right"></i>
    </button>
  </div>

  <!-- Panels -->
  <div class="panels">
    <div class="panel-left" style="width: {leftWidth}px;">
      <ChapterNav
        {chapters}
        {activeTabId}
        onselect={selectChapter}
        onadd={handleAddChapter}
        onrename={handleRenameChapter}
        ondelete={handleDeleteChapter}
        {wordCount}
        {chapterCounts}
      />
    </div>

    <div
      class="resize-handle"
      class:active={dragging === 'left'}
      role="separator"
      aria-orientation="vertical"
      onmousedown={() => startDrag('left')}
    ></div>

    <div class="panel-center">
      <Editor
        bind:this={editorRef}
        {openTabs}
        {activeTabId}
        chapter={activeChapter}
        onselecttab={selectChapter}
        onclosetab={closeTab}
        onchange={handleContentChange}
        onselectionchange={text => { selectedText = text; if (text.trim()) lastSelectedText = text; }}
        onentityclick={handleEntityClick}
        onquickadd={handleQuickAdd}
        {viewedEntityIds}
      />
    </div>

    <div
      class="resize-handle"
      class:active={dragging === 'right'}
      role="separator"
      aria-orientation="vertical"
      onmousedown={() => startDrag('right')}
    ></div>

    <div class="panel-right" style="width: {rightWidth}px;">
      <div class="panel-tabs">
        <button class="panel-tab" class:active={rightPanelTab === 'entities'} onclick={() => rightPanelTab = 'entities'}>
          <i class="bi bi-people"></i> Entities
        </button>
        <button class="panel-tab" class:active={rightPanelTab === 'search'} onclick={() => rightPanelTab = 'search'}>
          <i class="bi bi-search"></i> Search
        </button>
        <button class="panel-tab" class:active={rightPanelTab === 'analysis'} onclick={() => rightPanelTab = 'analysis'}>
          <i class="bi bi-bar-chart-line"></i> Analysis
        </button>
      </div>

      {#if rightPanelTab === 'entities'}
        <EntityPanel
          {entities}
          selectedText={selectedText || lastSelectedText}
          {pendingEntityName}
          {focusEntityId}
          {focusTrigger}
          activeChapterId={activeChapter?.id}
          oncreate={async (name, type, desc, color) => { pendingEntityName = null; await handleCreateEntity(name, type, desc, color); }}
          onupdate={handleUpdateEntity}
          ondelete={handleDeleteEntity}
          onaliasadd={handleAddAlias}
          onaliasremove={handleRemoveAlias}
          ontoggleview={toggleViewEntity}
          ongotochapter={handleGoToChapter}
          onclearselection={() => { selectedText = ''; lastSelectedText = ''; editorRef?.clearSelection(); }}
        />
      {:else if rightPanelTab === 'search'}
        <SearchPanel
          {entities}
          ongotochapter={handleGoToChapter}
        />
      {:else if rightPanelTab === 'analysis'}
        <AnalysisPanel
          {entities}
          ongotochapter={handleGoToChapter}
        />
      {/if}
    </div>
  </div>

  <!-- Status bar -->
  <div class="statusbar">
    {#if activeChapter}
      <span class="status-chapter">{activeChapter.title}</span>
      <span class="status-sep">&middot;</span>
      <span>{currentWords.toLocaleString()} words</span>
    {/if}
    <span class="status-spacer"></span>
    <span>{totalWords.toLocaleString()} total</span>
  </div>
</div>

<Toasts />

<style>
  .workspace {
    display: flex; flex-direction: column; height: 100vh;
    background: var(--iwe-bg);
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
  .stats-btn {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.3rem 0.7rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: inline-flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .stats-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  .export-wrap { position: relative; }
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
  .nav-btn {
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text-secondary); padding: 0.25rem 0.5rem;
    display: inline-flex; align-items: center; justify-content: center;
    transition: all 150ms; font-size: 0.9rem;
  }
  .nav-btn:hover:not(:disabled) { background: var(--iwe-bg-hover); color: var(--iwe-text); border-color: var(--iwe-accent); }
  .nav-btn:disabled { opacity: 0.25; cursor: default; }

  .panels {
    display: flex; flex: 1; overflow: hidden;
  }

  .panel-left {
    flex-shrink: 0;
    border-right: none;
    background: var(--iwe-bg-sidebar); overflow-y: auto;
  }
  .panel-center {
    flex: 1; display: flex; flex-direction: column; overflow: hidden;
    background: var(--iwe-paper); min-width: 200px;
  }
  .panel-right {
    flex-shrink: 0;
    border-left: none;
    background: var(--iwe-bg-sidebar);
    display: flex; flex-direction: column; overflow: hidden;
  }

  .panel-tabs {
    display: flex; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
  }
  .panel-tab {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.35rem;
    padding: 0.5rem 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    background: none; border: none; border-bottom: 2px solid transparent;
    color: var(--iwe-text-muted); cursor: pointer;
    transition: all 150ms;
  }
  .panel-tab:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-hover); }
  .panel-tab.active {
    color: var(--iwe-text); border-bottom-color: var(--iwe-accent);
  }

  .search-panel, .analysis-panel {
    flex: 1; overflow-y: auto;
  }

  /* Resize handles */
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

  .statusbar {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0 1rem; height: 26px; flex-shrink: 0;
    border-top: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
    font-size: 0.75rem; color: var(--iwe-text-muted);
  }
  .status-chapter { font-weight: 500; }
  .status-sep { color: var(--iwe-text-faint); }
  .status-spacer { flex: 1; }
</style>
