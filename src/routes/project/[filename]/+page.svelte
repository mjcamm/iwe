<script>
  import { onMount } from 'svelte';
  import { beforeNavigate } from '$app/navigation';
  import { getSettings, saveSettings, getChapters, getChapter, addChapter, updateChapterContent, renameChapter, deleteChapter, getEntities, createEntity, updateEntity, deleteEntity, setEntityVisible, addAlias, removeAlias, scanAllChapters, getNavHistory, pushNavEntry, truncateNavAfter, addEntityNote, logWritingActivity, setSpellLanguage, getAllChapterWordCounts, getChapterComments, addComment, updateComment, deleteComment, addStateMarker, deleteStateMarker, getStateMarker, getChapterDialogue } from '$lib/db.js';
  import ChapterNav from '$lib/components/ChapterNav.svelte';
  import Editor from '$lib/components/Editor.svelte';
  import EntityPanel from '$lib/components/EntityPanel.svelte';
  import NotesPanel from '$lib/components/NotesPanel.svelte';
  import { buildTextMap } from '$lib/entityHighlight.js';
  import SearchPanel from '$lib/components/SearchPanel.svelte';
  import AnalysisPanel from '$lib/components/AnalysisPanel.svelte';
  import { addToast } from '$lib/toast.js';

  let { data } = $props();

  let projectLoading = $state(true);
  let editorReady = $state(false);
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
  let selectionFrom = 0;
  let selectionTo = 0;
  let editorRef;

  // Dialogue detection highlight state
  let dialogueHighlightActive = $state(false);

  const DIALOGUE_OPENERS = new Set(['"', '\u201C', '\u201E', '\u00AB', '\u300C']);
  const DIALOGUE_CLOSER_MAP = {
    '"': new Set(['"']),
    '\u201C': new Set(['\u201D', '"']),
    '\u201E': new Set(['\u201D', '\u201C']),
    '\u00AB': new Set(['\u00BB']),
    '\u300C': new Set(['\u300D']),
  };

  function computeDialogueRanges(ed) {
    const { text, posMap } = buildTextMap(ed.state.doc);
    const ranges = [];
    let i = 0;
    while (i < text.length) {
      const ch = text[i];
      if (DIALOGUE_OPENERS.has(ch)) {
        if (ch === '"') {
          const next = i + 1 < text.length ? text[i + 1] : ' ';
          if (!/[a-zA-Z\u00C0-\u024F\u2014'\u2018\u2019]/.test(next)) {
            i++;
            continue;
          }
        }
        const validClosers = DIALOGUE_CLOSER_MAP[ch];
        let j = i + 1;
        let found = false;
        while (j < text.length && j - i < 500) {
          if (validClosers.has(text[j])) {
            if (j - i > 2) {
              ranges.push({ from: posMap[i], to: posMap[i] + 1, class: 'debug-highlight-quote' });
              if (i + 1 < j) {
                ranges.push({ from: posMap[i + 1], to: posMap[j - 1] + 1, class: 'debug-highlight-inner' });
              }
              ranges.push({ from: posMap[j], to: posMap[j] + 1, class: 'debug-highlight-quote' });
            }
            i = j + 1;
            found = true;
            break;
          }
          j++;
        }
        if (found) continue;
      }
      i++;
    }
    return ranges;
  }

  function refreshDialogueHighlight() {
    if (!dialogueHighlightActive) return;
    const ed = editorRef?.getEditor();
    if (!ed) return;
    editorRef?.setDebugDecorations(computeDialogueRanges(ed));
  }

  function toggleDialogueHighlight() {
    const ed = editorRef?.getEditor();
    if (!ed || !activeChapter) return;

    if (dialogueHighlightActive) {
      editorRef?.setDebugDecorations([]);
      dialogueHighlightActive = false;
      return;
    }

    editorRef?.setDebugDecorations(computeDialogueRanges(ed));
    dialogueHighlightActive = true;
  }

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
    // Load saved panel widths
    const settings = await getSettings();
    if (settings.leftPanelWidth) leftWidth = settings.leftPanelWidth;
    if (settings.rightPanelWidth) rightWidth = settings.rightPanelWidth;
    if (settings.spellLanguage) {
      try { await setSpellLanguage(settings.spellLanguage); } catch {}
    }

    projectTitle = data.filename.replace('.iwe', '');
    chapters = await getChapters();
    entities = await getEntities();
    refreshChapterCounts();

    // Initialize word count tracking from Rust (Y.Doc)
    try {
      const counts = await getAllChapterWordCounts();
      const wc = {};
      for (const [id, count] of counts) {
        lastKnownWordCounts[id] = count;
        wc[id] = count;
      }
      wordCounts = wc;
    } catch (e) {
      console.warn('[stats] failed to load word counts:', e);
    }

    // Load nav history
    navHistory = await getNavHistory();
    navIndex = navHistory.length > 0 ? navHistory.length - 1 : -1;

    if (chapters.length > 0) {
      await selectChapter(chapters[0].id);
    }

    projectLoading = false;

    // Listen for cross-window navigation events (forwarded by layout)
    window.addEventListener('iwe-navigate-to-position', (event) => {
      const p = event.detail;
      handleGoToChapter(p.chapterId, p.searchText, p.charPosition || 0);
    });

    // Set up scroll listener for auto-checkpointing
    setTimeout(() => {
      const scrollEl = document.querySelector('.editor-scroll');
      if (scrollEl) {
        scrollEl.addEventListener('scroll', () => resetCheckpointTimer());
      }
    }, 500);
  });

  // Flush pending autosave before navigating away
  beforeNavigate(() => {
    if (saveTimer && activeChapter) {
      clearTimeout(saveTimer);
      saveTimer = null;
      updateChapterContent(activeChapter.id, activeChapter.content);
    }
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
      // Ctrl+click with selection → pin text range to entity using Y.Doc relative positions
      const relPositions = editorRef?.createRelativePositions(selectionFrom, selectionTo);
      if (!relPositions) return;
      await addEntityNote(entityId, activeChapter?.id || null, relPositions.yStart, relPositions.yEnd);
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
  let focusStateTab = $state(0);
  let focusStateId = $state(null);
  let cursorMoveTrigger = $state(0);
  let cursorPos = $state(0);
  let cursorMoveTimer = null;
  let rightPanelTab = $state('entities'); // 'entities' | 'search' | 'analysis' | 'notes'
  let chapterComments = $state([]); // comments from DB for current chapter
  let resolvedComments = $state([]); // comments with positions from editor
  let activeNoteId = $state(null); // selected note in detail view

  let pendingEntityName = $state(null);

  function handleQuickAdd(word) {
    pendingEntityName = word;
  }

  /**
   * Universal jump-to-position. Used by EVERYTHING.
   *
   * Position resolution uses buildTextMap as the single source of truth.
   * Rust char_position values are used only as proximity hints for disambiguation,
   * never as direct indices into posMap.
   *
   * @param chapterId - which chapter to open
   * @param searchText - the word/phrase to find and highlight
   * @param positionHint - Rust char_position (number) as proximity hint for disambiguation,
   *                       or {pmFrom, pmTo} for pre-resolved PM positions (entity notes)
   */
  /**
   * Find all occurrences of needle in text, return the one closest to hintCharPos.
   * Returns { start, end } char indices in the buildTextMap text, or null.
   */
  function findTextNearHint(text, needle, hintCharPos) {
    if (!needle || !text) return null;

    // Try case-sensitive first, then case-insensitive
    const lowerText = text.toLowerCase();
    const lowerNeedle = needle.toLowerCase();
    const searches = [
      { haystack: text, needle: needle },          // exact case
      { haystack: lowerText, needle: lowerNeedle }, // case insensitive
    ];

    for (const { haystack, needle: n } of searches) {
      // Find all occurrences
      const occurrences = [];
      let pos = 0;
      while (true) {
        const idx = haystack.indexOf(n, pos);
        if (idx === -1) break;
        occurrences.push(idx);
        pos = idx + 1;
      }

      if (occurrences.length === 0) continue;

      // Pick the occurrence nearest to the hint position
      let best = occurrences[0];
      let bestDist = Math.abs(best - hintCharPos);
      for (const occ of occurrences) {
        const dist = Math.abs(occ - hintCharPos);
        if (dist < bestDist) {
          best = occ;
          bestDist = dist;
        }
      }

      return { start: best, end: best + needle.length };
    }

    return null;
  }

  async function handleGoToChapter(chapterId, searchText, positionHint) {
    await pushNavCheckpoint(true);
    await selectChapter(chapterId);
    setTimeout(() => {
      const ed = editorRef?.getEditor();
      if (!ed) return;

      let pmFrom = null;
      let pmTo = null;

      if (positionHint && typeof positionHint === 'object' && 'pmFrom' in positionHint) {
        // Pre-resolved PM positions (from Y.Doc relative positions — entity notes)
        pmFrom = positionHint.pmFrom;
        pmTo = positionHint.pmTo;

      } else if (searchText) {
        // Find searchText in buildTextMap text — the single source of truth.
        // Rust char_position is used only as a proximity hint for disambiguation.
        const { text, posMap } = buildTextMap(ed.state.doc);
        const hint = typeof positionHint === 'number' ? positionHint : 0;
        const found = findTextNearHint(text, searchText, hint);

        if (found !== null) {
          pmFrom = posMap[found.start];
          pmTo = posMap[found.end - 1] + 1;
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
        setTimeout(() => container.remove(), 3000);
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

  // ---- Comments / notes ----

  async function loadComments() {
    if (!activeChapter) { resolvedComments = []; return; }
    try {
      chapterComments = await getChapterComments(activeChapter.id);
    } catch (e) {
      console.warn('[comments] load failed:', e);
      chapterComments = [];
    }
    refreshResolvedComments();
    // Apply highlight decorations in the editor
    editorRef?.setActiveComment(activeNoteId);
  }

  function refreshResolvedComments() {
    const markers = editorRef?.getNoteMarkerPositions() || [];
    const markerMap = new Map(markers.map(m => [m.commentId, m]));
    resolvedComments = chapterComments
      .map(c => ({
        ...c,
        pos: markerMap.get(c.id)?.pos ?? -1,
        highlightLen: markerMap.get(c.id)?.highlightLen ?? 0,
      }))
      .sort((a, b) => {
        if (a.pos === -1 && b.pos === -1) return 0;
        if (a.pos === -1) return 1;
        if (b.pos === -1) return -1;
        return a.pos - b.pos;
      });
  }

  async function handleAddComment({ from, highlightLen }) {
    if (!activeChapter) return;
    try {
      const id = await addComment(activeChapter.id, '');
      editorRef?.insertNoteMarker(from, id, highlightLen);
      await loadComments();
      rightPanelTab = 'notes';
      activeNoteId = id;
    } catch (e) {
      console.error('[comments] add failed:', e);
    }
  }

  async function handleUpdateComment(id, noteText) {
    try {
      await updateComment(id, noteText);
      // Update local state
      chapterComments = chapterComments.map(c => c.id === id ? { ...c, note_text: noteText } : c);
      refreshResolvedComments();
    } catch (e) {
      console.error('[comments] update failed:', e);
    }
  }

  async function handleDeleteComment(id) {
    try {
      editorRef?.removeNoteMarker(id);
      await deleteComment(id);
      activeNoteId = null;
      await loadComments();
    } catch (e) {
      console.error('[comments] delete failed:', e);
    }
  }

  function handleCommentClick(commentId) {
    rightPanelTab = 'notes';
    activeNoteId = commentId;
    editorRef?.setActiveComment(commentId);
  }

  function handleSelectNote(commentId) {
    activeNoteId = commentId;
    editorRef?.setActiveComment(commentId);
    if (commentId != null) {
      editorRef?.scrollToNoteMarker(commentId);
    }
  }

  function handleUpdateHighlight(commentId) {
    const ed = editorRef?.getEditor();
    if (!ed) return;
    const { from, to } = ed.state.selection;
    const highlightLen = (from !== to) ? (to - from) : 0;
    editorRef?.moveNoteMarker(commentId, from, highlightLen);
    refreshResolvedComments();
  }

  // ---- Entity state tracking ----

  async function handleAddStateFact({ from, entityId, entityName }) {
    if (!activeChapter) return;
    try {
      const id = await addStateMarker(entityId, activeChapter.id);
      editorRef?.insertStateMarker(from, id, 'fact');
      // Open entity panel to state tab with this marker selected for editing
      // buildEditRows will show all existing entity keys with resolved values
      focusEntityId = entityId;
      focusTrigger++;
      focusStateTab++;
      focusStateId = id;
      rightPanelTab = 'entities';
    } catch (e) {
      console.error('[entity-state] add marker failed:', e);
    }
  }

  function getTimeSectionForPos(chapterId, docPos) {
    // Only works for the currently open chapter
    if (chapterId !== activeChapter?.id) return { chapterId, sectionIndex: 0 };
    const ed = editorRef?.getEditor();
    if (!ed) return { chapterId, sectionIndex: 0 };

    // Walk top-level nodes to build sections matching Rust extract_time_sections:
    // Flow blocks accumulate into a flow section. A timeBreak wrapper is its own section.
    // Section indices: flow(0), timeBreak(1), flow(2), timeBreak(3), flow(4)...
    const doc = ed.state.doc;
    let sectionIndex = 0;
    let hasFlowInCurrentSection = false;
    let result = { chapterId, sectionIndex: 0 };
    let found = false;

    doc.forEach((node, offset) => {
      if (found) return;
      const nodeEnd = offset + node.nodeSize;

      if (node.type.name === 'timeBreak') {
        // Finalize preceding flow section if it had content
        if (hasFlowInCurrentSection) {
          sectionIndex++;
          hasFlowInCurrentSection = false;
        } else if (sectionIndex === 0) {
          // Empty flow before first timeBreak still counts as section 0
          sectionIndex++;
        }

        // Check if pos is inside this timeBreak
        if (docPos >= offset && docPos < nodeEnd) {
          result = { chapterId, sectionIndex };
          found = true;
          return;
        }
        sectionIndex++;
      } else {
        // Flow block
        hasFlowInCurrentSection = true;
        if (docPos >= offset && docPos < nodeEnd) {
          result = { chapterId, sectionIndex };
          found = true;
          return;
        }
      }
    });

    return result;
  }

  function handleDeleteState(stateId) {
    editorRef?.removeStateMarker(stateId);
  }

  async function handleNavToMarker(markerId, chapterId) {
    // Open the chapter if needed, then scroll to the marker
    if (activeChapter?.id !== chapterId) {
      await selectChapter(chapterId);
      // Wait for editor to load
      await new Promise(r => setTimeout(r, 200));
    }
    const positions = editorRef?.getStateMarkerPositions() || [];
    const found = positions.find(p => p.stateId === markerId);
    if (found) {
      const ed = editorRef?.getEditor();
      if (ed) {
        ed.chain().focus().setTextSelection(found.pos).run();
        setTimeout(() => {
          const scrollEl = document.querySelector('.editor-scroll');
          if (!scrollEl || !ed.view) return;
          const coords = ed.view.coordsAtPos(found.pos);
          const rect = scrollEl.getBoundingClientRect();
          const target = coords.top - rect.top + scrollEl.scrollTop - rect.height / 3;
          scrollEl.scrollTo({ top: target, behavior: 'smooth' });
        }, 50);
      }
    }
  }

  async function handleStateMarkerClick(stateId, stateType) {
    try {
      const marker = await getStateMarker(stateId);
      if (!marker) return;
      focusEntityId = marker.entity_id;
      focusTrigger++;
      focusStateTab++;
      focusStateId = stateId;
      rightPanelTab = 'entities';
    } catch (e) {
      console.error('[entity-state] marker click failed:', e);
    }
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
    // Load comments after a short delay to let editor mount
    setTimeout(() => loadComments(), 300);
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

  // Word count cache: chapterId -> count (updated on save and init)
  let wordCounts = $state({});

  let dialogueRefreshTimer = null;

  function handleContentChange(content) {
    if (!activeChapter) return;
    activeChapter = { ...activeChapter, content };
    openTabs = openTabs.map(t => t.id === activeChapter.id ? { ...t, content } : t);

    // Refresh dialogue highlighting on edit (debounced)
    if (dialogueHighlightActive) {
      clearTimeout(dialogueRefreshTimer);
      dialogueRefreshTimer = setTimeout(refreshDialogueHighlight, 300);
    }

    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      updateChapterContent(activeChapter.id, content);
      refreshChapterCounts();

      // Track writing activity — get word count from editor
      const ed = editorRef?.getEditor();
      const currentWords = ed ? countWordsFromDoc(ed.state.doc) : 0;
      const lastWords = lastKnownWordCounts[activeChapter.id] || 0;
      const delta = currentWords - lastWords;
      lastKnownWordCounts[activeChapter.id] = currentWords;
      wordCounts = { ...wordCounts, [activeChapter.id]: currentWords };

      if (delta !== 0) {
        const manuscriptTotal = Object.values(wordCounts).reduce((a, b) => a + b, 0);
        logWritingActivity(activeChapter.id, currentWords, manuscriptTotal, delta).catch(e => console.error('[stats] log failed:', e));
      }
    }, 500);
  }

  function countWordsFromDoc(doc) {
    const text = doc.textContent;
    if (!text || !text.trim()) return 0;
    return text.trim().split(/\s+/).length;
  }

  let totalWords = $derived(Object.values(wordCounts).reduce((a, b) => a + b, 0));
  let currentWords = $derived(activeChapter ? (wordCounts[activeChapter.id] || 0) : 0);
</script>

{#if projectLoading || !editorReady}
  <div class="project-loading">
    <div class="project-loading-inner">
      <div class="loader"></div>
      <p class="loading-text">Loading your manuscript...</p>
    </div>
  </div>
{/if}
{#if !projectLoading}
<div class="workspace">
  <!-- Editor nav bar -->
  <div class="editor-nav">
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
        onselectionchange={(text, from, to) => {
          selectedText = text;
          if (text.trim()) {
            lastSelectedText = text;
            selectionFrom = from;
            selectionTo = to;
          }
          clearTimeout(cursorMoveTimer);
          cursorMoveTimer = setTimeout(() => { cursorMoveTrigger++; }, 300);
          if (from !== undefined) cursorPos = from;
        }}
        onentityclick={handleEntityClick}
        onquickadd={handleQuickAdd}
        onready={() => { editorReady = true; loadComments(); }}
        onaddcomment={handleAddComment}
        oncommentclick={handleCommentClick}
        onaddstatefact={handleAddStateFact}
        onstateclick={handleStateMarkerClick}
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
        <button class="panel-tab" class:active={rightPanelTab === 'notes'} onclick={() => rightPanelTab = 'notes'}>
          <i class="bi bi-chat-left-text"></i> Notes
          {#if resolvedComments.length > 0}
            <span class="panel-tab-count">{resolvedComments.length}</span>
          {/if}
        </button>
      </div>

      {#if rightPanelTab === 'entities'}
        <EntityPanel
          {entities}
          {chapters}
          selectedText={selectedText || lastSelectedText}
          {pendingEntityName}
          {focusEntityId}
          {focusTrigger}
          {focusStateTab}
          {focusStateId}
          {cursorMoveTrigger}
          {cursorPos}
          getMarkerPositions={() => editorRef?.getStateMarkerPositions() || []}
          {getTimeSectionForPos}
          activeChapterId={activeChapter?.id}
          oncreate={async (name, type, desc, color) => { pendingEntityName = null; await handleCreateEntity(name, type, desc, color); }}
          onupdate={handleUpdateEntity}
          ondelete={handleDeleteEntity}
          onaliasadd={handleAddAlias}
          onaliasremove={handleRemoveAlias}
          ontoggleview={toggleViewEntity}
          ongotochapter={handleGoToChapter}
          onclearselection={() => { selectedText = ''; lastSelectedText = ''; editorRef?.clearSelection(); }}
          ondeletestate={handleDeleteState}
          onnavtomarker={handleNavToMarker}
          resolveNotePositions={(yStart, yEnd) => editorRef?.resolveRelativePositions(yStart, yEnd)}
          getEditorTextBetween={(from, to) => editorRef?.getTextBetween(from, to)}
          createNotePositions={() => editorRef?.createRelativePositions(selectionFrom, selectionTo)}
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
          ontoggledialoguehighlight={toggleDialogueHighlight}
          dialogueHighlightActive={dialogueHighlightActive}
        />
      {:else if rightPanelTab === 'notes'}
        <NotesPanel
          comments={resolvedComments}
          {activeNoteId}
          hasEditorSelection={!!selectedText?.trim()}
          ondelete={handleDeleteComment}
          onupdate={handleUpdateComment}
          onselectnote={handleSelectNote}
          onupdatehighlight={handleUpdateHighlight}
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
{/if}

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
    width: 45px;
    aspect-ratio: .75;
    --c: no-repeat linear-gradient(var(--iwe-accent) 0 0);
    background:
      var(--c) 0%   50%,
      var(--c) 50%  50%,
      var(--c) 100% 50%;
    background-size: 20% 50%;
    animation: l6 1s infinite linear;
  }
  @keyframes l6 {
    20% {background-position: 0% 0%  ,50% 50% ,100% 50% }
    40% {background-position: 0% 100%,50% 0%  ,100% 50% }
    60% {background-position: 0% 50% ,50% 100%,100% 0%  }
    80% {background-position: 0% 50% ,50% 50% ,100% 100%}
  }
  .loading-text {
    font-family: var(--iwe-font-prose); font-size: 0.95rem;
    color: var(--iwe-text-muted); font-style: italic; margin: 0;
  }

  .workspace {
    display: flex; flex-direction: column; height: 100%;
    background: var(--iwe-bg);
  }

  .editor-nav {
    display: flex; align-items: center; gap: 0.3rem;
    padding: 0.2rem 0.5rem; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border-light);
    background: var(--iwe-bg-warm);
  }
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
    font-size: 0.9rem;
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
