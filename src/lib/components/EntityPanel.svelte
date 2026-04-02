<script>
  import { detectEntities, addIgnoredWord, findReferences, getEntityNotes, addEntityNote, deleteEntityNote, reorderEntityNotes, getEntityFreeNotes, addEntityFreeNote, updateEntityFreeNote, deleteEntityFreeNote, reorderEntityFreeNotes, getEntityMarkers, addStateMarkerValue, updateStateMarkerValue, deleteStateMarkerValue, deleteStateMarker, getDistinctStateKeys, getEntityStateKeys, addStateMarkerEntityRef, updateStateMarkerEntityRef, getIncomingEntityRefs, getTimeSectionOrder } from '$lib/db.js';
  import { addToast } from '$lib/toast.js';
  import { dndzone } from 'svelte-dnd-action';

  let {
    entities = [],
    selectedText = '',
    oncreate,
    onupdate,
    ondelete,
    onaliasadd,
    onaliasremove,
    ontoggleview,
    ongotochapter,
    pendingEntityName = null,
    focusEntityId = null,
    focusTrigger = 0,
    activeChapterId = null,
    onctrlclickentity,
    onclearselection,
    ondeletestate,
    onnavtomarker,
    chapters = [],
    focusStateTab = 0,
    focusStateId = null,
    cursorMoveTrigger = 0,
    cursorPos = 0,
    getMarkerPositions = null, // function that returns [{stateId, pos}] from editor
    getTimeSectionForPos = null, // function(chapterId, docPos) => {chapterId, sectionIndex}
  } = $props();

  let view = $state('list'); // 'list' | 'create' | 'detect' | 'references' | 'view'
  let editingEntity = $state(null);
  let filterText = $state('');
  let filterType = $state('all');

  // Create form
  let newName = $state('');
  let newType = $state('character');
  let newDescription = $state('');
  let newColor = $state('');

  // Edit form
  let editName = $state('');
  let editType = $state('character');
  let editDescription = $state('');
  let editColor = $state('');
  let newAlias = $state('');

  const defaultColors = { character: '#2d6a5e', place: '#6a4c2d', thing: '#4c2d6a' };

  // Open create form when triggered from outside (e.g. live suggestion bubble)
  $effect(() => {
    if (pendingEntityName) {
      newName = pendingEntityName;
      newType = 'character';
      newDescription = '';
      newColor = defaultColors['character'];
      view = 'create';
    }
  });

  // Open entity view when clicked in editor
  $effect(() => {
    // Read trigger to force reactivity even for same entity
    const _trigger = focusTrigger;
    if (focusEntityId) {
      const entity = entities.find(e => e.id === focusEntityId);
      if (entity) {
        showEntityView(entity);
      }
    }
  });

  // Switch to state tab when triggered from outside
  $effect(() => {
    const _t = focusStateTab;
    if (_t > 0) {
      viewTab = 'state';
    }
  });

  // Build edit rows when activeMarkerId changes — only on initial open, not on data refreshes
  let lastBuiltMarkerId = null;
  $effect(() => {
    const mid = activeMarkerId;
    if (mid && mid !== lastBuiltMarkerId && viewEntity && viewMarkers.length > 0) {
      lastBuiltMarkerId = mid;
      showDeleteMarkerConfirm = false;
      buildEditRows(mid, viewEntity.id);
    }
    if (!mid) {
      lastBuiltMarkerId = null;
      showDeleteMarkerConfirm = false;
    }
  });

  // Build a story-order map: (chapterId, sectionIndex) => storyOrder
  function buildStoryOrderMap() {
    const map = new Map(); // "chId-secIdx" => storyOrder
    if (timeSectionOrder.length > 0) {
      for (const entry of timeSectionOrder) {
        map.set(`${entry.chapter_id}-${entry.section_index}`, entry.story_order);
      }
    }
    // Fallback for chapters/sections not in the map: chapter sort_order * 1000 + section_index
    return map;
  }

  function getStoryOrder(storyMap, chapterId, sectionIndex) {
    const key = `${chapterId}-${sectionIndex}`;
    if (storyMap.has(key)) return storyMap.get(key);
    const ch = chapters.find(c => c.id === chapterId);
    return (ch?.sort_order ?? 0) * 1000 + sectionIndex;
  }

  // Resolve state up to a given doc position in a given chapter, using story-time ordering
  function resolveUpTo(chapterId, docPos) {
    const markerPositions = getMarkerPositions?.() || [];
    const posMap = new Map();
    for (const mp of markerPositions) {
      posMap.set(mp.stateId, mp.pos);
    }

    const storyMap = buildStoryOrderMap();

    // Determine the target's story-time position
    const targetSection = getTimeSectionForPos?.(chapterId, docPos) ?? { chapterId, sectionIndex: 0 };
    const targetStoryOrder = getStoryOrder(storyMap, targetSection.chapterId, targetSection.sectionIndex);

    const before = [];
    for (const m of viewMarkers) {
      // For markers in the current chapter, we know their doc position
      if (m.chapter_id === chapterId) {
        const mPos = posMap.get(m.id);
        if (mPos === undefined) continue;

        // Determine which section this marker is in
        const mSection = getTimeSectionForPos?.(chapterId, mPos) ?? { chapterId, sectionIndex: 0 };
        const mStoryOrder = getStoryOrder(storyMap, mSection.chapterId, mSection.sectionIndex);

        // Include if: earlier in story time, OR same section and earlier doc position
        if (mStoryOrder < targetStoryOrder) {
          before.push({ marker: m, order: mStoryOrder * 100000 + mPos });
        } else if (mStoryOrder === targetStoryOrder && mPos <= docPos) {
          before.push({ marker: m, order: mStoryOrder * 100000 + mPos });
        }
      } else {
        // Marker in a different chapter — use its chapter's default section 0
        // (we don't have doc positions for markers in other chapters)
        const mStoryOrder = getStoryOrder(storyMap, m.chapter_id, 0);
        if (mStoryOrder < targetStoryOrder) {
          before.push({ marker: m, order: mStoryOrder * 100000 });
        } else if (mStoryOrder === targetStoryOrder) {
          // Same story order but different chapter — include (it's at the same story time)
          before.push({ marker: m, order: mStoryOrder * 100000 });
        }
      }
    }
    before.sort((a, b) => a.order - b.order);

    const facts = new Map();
    const entityRefs = new Map();
    for (const { marker } of before) {
      for (const v of marker.values) {
        if (v.value_type === 'entity_ref' && v.ref_entity_id) {
          entityRefs.set(v.ref_entity_id, { name: v.ref_entity_name || 'Unknown', active: v.ref_active });
        } else if (v.fact_key) {
          facts.set(v.fact_key, v.fact_value);
        }
      }
    }
    return { facts, entityRefs };
  }

  async function buildEditRows(markerId, entityId) {
    try {
      const allKeys = await getEntityStateKeys(entityId);

      const marker = viewMarkers.find(m => m.id === markerId);
      const markerVals = marker ? marker.values : [];

      // Resolve state up to just BEFORE this marker's position (so we see inherited state, not this marker's own values)
      const markerPositions = getMarkerPositions?.() || [];
      const markerDocPos = markerPositions.find(p => p.stateId === markerId)?.pos ?? 0;
      const { facts: resolvedMap, entityRefs: resolvedRefs } = resolveUpTo(marker?.chapter_id || activeChapterId, markerDocPos - 1);

      const rows = [];
      const seen = new Set();

      // Values this marker explicitly sets
      for (const v of markerVals) {
        if (v.value_type === 'entity_ref') {
          rows.push({
            type: 'entity_ref',
            valueId: v.id,
            refEntityId: v.ref_entity_id,
            refEntityName: v.ref_entity_name || 'Unknown',
            refActive: v.ref_active,
            isSet: true,
          });
        } else {
          rows.push({
            type: 'fact',
            key: v.fact_key,
            value: v.fact_value,
            valueId: v.id,
            resolvedValue: resolvedMap.get(v.fact_key) ?? '',
            isSet: true,
          });
          seen.add(v.fact_key);
        }
      }

      // Fact keys from other checkpoints not set here
      for (const key of allKeys) {
        if (!seen.has(key)) {
          rows.push({
            type: 'fact',
            key,
            value: resolvedMap.get(key) ?? '',
            valueId: null,
            resolvedValue: resolvedMap.get(key) ?? '',
            isSet: false,
          });
          seen.add(key);
        }
      }

      // Also include entity refs from other markers that this one doesn't set
      const allMarkerVals = viewMarkers.flatMap(m => m.values);
      const thisRefIds = new Set(markerVals.filter(v => v.value_type === 'entity_ref').map(v => v.ref_entity_id));
      const seenRefIds = new Set(thisRefIds);
      for (const v of allMarkerVals) {
        if (v.value_type === 'entity_ref' && v.ref_entity_id && !seenRefIds.has(v.ref_entity_id)) {
          const resolved = resolvedRefs.get(v.ref_entity_id);
          rows.push({
            type: 'entity_ref',
            valueId: null,
            refEntityId: v.ref_entity_id,
            refEntityName: v.ref_entity_name || 'Unknown',
            refActive: resolved ? resolved.active : false,
            isSet: false,
          });
          seenRefIds.add(v.ref_entity_id);
        }
      }

      rows.sort((a, b) => {
        const aName = (a.type === 'fact' ? a.key : a.refEntityName) || '';
        const bName = (b.type === 'fact' ? b.key : b.refEntityName) || '';
        return aName.localeCompare(bName, undefined, { sensitivity: 'base' });
      });
      editRows = rows;
      addingEntityRef = false;
    } catch (e) {
      console.error('[entity-state] buildEditRows failed:', e);
      editRows = [];
    }
  }

  // Resolve state locally when cursor moves, using marker positions from editor
  $effect(() => {
    const _cursor = cursorMoveTrigger;
    const _pos = cursorPos;
    if (viewTab === 'state' && viewEntity && !activeMarkerId) {
      resolveStateAtCursor();
    }
  });

  function resolveStateAtCursor() {
    const { facts, entityRefs } = resolveUpTo(activeChapterId, cursorPos);
    const factList = Array.from(facts.entries()).map(([key, value]) => ({ key, value }));
    factList.sort((a, b) => a.key.localeCompare(b.key, undefined, { sensitivity: 'base' }));
    const refList = Array.from(entityRefs.entries()).map(([id, data]) => ({ refEntityId: id, refEntityName: data.name, refActive: data.active }));
    refList.sort((a, b) => a.refEntityName.localeCompare(b.refEntityName, undefined, { sensitivity: 'base' }));
    resolvedState = { facts: factList, entityRefs: refList };

    // Resolve incoming refs at cursor position
    const markerPositions = getMarkerPositions?.() || [];
    const posMap = new Map();
    for (const mp of markerPositions) posMap.set(mp.stateId, mp.pos);
    const currentChSortOrder = chapters.find(c => c.id === activeChapterId)?.sort_order ?? 0;

    // Group incoming refs by source entity — take latest before cursor per source
    const incomingMap = new Map(); // srcId -> { name, active }
    // Sort raw refs by position (chapter order, then marker doc position)
    const sorted = [...incomingRefsRaw].map(([srcId, srcName, active, chId, markerId]) => {
      const chSort = chapters.find(c => c.id === chId)?.sort_order ?? 0;
      const docPos = posMap.get(markerId);
      return { srcId, srcName, active, chId, markerId, chSort, docPos };
    }).sort((a, b) => {
      if (a.chSort !== b.chSort) return a.chSort - b.chSort;
      return (a.docPos ?? 0) - (b.docPos ?? 0);
    });

    for (const ref of sorted) {
      // Only include if before cursor
      if (ref.chSort < currentChSortOrder) {
        incomingMap.set(ref.srcId, { name: ref.srcName, active: ref.active });
      } else if (ref.chId === activeChapterId && ref.docPos !== undefined && ref.docPos <= cursorPos) {
        incomingMap.set(ref.srcId, { name: ref.srcName, active: ref.active });
      }
    }

    resolvedIncoming = Array.from(incomingMap.entries())
      .map(([id, data]) => ({ srcId: id, srcName: data.name, active: data.active }))
      .sort((a, b) => a.srcName.localeCompare(b.srcName, undefined, { sensitivity: 'base' }));
  }

  // Detection state
  let candidates = $state([]);
  let detecting = $state(false);
  let browsingCandidate = $state(null);
  let browseIndex = $state(0);
  let linkingCandidate = $state(null); // candidate text being linked as alias

  // References state
  let refsEntity = $state(null); // the entity we're showing references for
  let refsData = $state(null); // EntityReferences result
  let refsLoading = $state(false);

  // View entity state
  let viewEntity = $state(null);
  let viewNotes = $state([]);
  let viewFreeNotes = $state([]);
  let viewNotesLoading = $state(false);
  let viewTab = $state('excerpts'); // 'excerpts' | 'notes' | 'state'
  let newNoteText = $state('');

  // Entity state tracking (checkpoint model)
  let viewMarkers = $state([]); // StateMarker[] with .values
  let stateKeySuggestions = $state([]);
  let activeMarkerId = $state(null); // which marker is being edited
  let resolvedState = $state(null); // { facts: [] }
  let editRows = $state([]); // merged rows for checkpoint editor
  let addingEntityRef = $state(false);
  let incomingRefsRaw = $state([]); // all incoming refs from DB: [srcId, srcName, active, chId, markerId]
  let resolvedIncoming = $state([]); // cursor-position-resolved incoming refs
  let stateFilterKey = $state('');
  let stateFilterValue = $state('');
  let timeSectionOrder = $state([]); // from DB, for story-time ordering
  let showDeleteMarkerConfirm = $state(false);
  let stateSubView = $state('cursor'); // 'cursor' | 'all'

  async function showEntityView(entity) {
    viewEntity = entity;
    editingEntity = entity;
    editName = entity.name;
    editType = entity.entity_type;
    editColor = entity.color || defaultColors[entity.entity_type];
    newAlias = '';
    showDeleteConfirm = false;
    viewNotesLoading = true;
    activeMarkerId = null;
    view = 'view';
    try {
      viewNotes = await getEntityNotes(entity.id);
      viewFreeNotes = await getEntityFreeNotes(entity.id);
      viewMarkers = await getEntityMarkers(entity.id);
      stateKeySuggestions = await getDistinctStateKeys();
      incomingRefsRaw = await getIncomingEntityRefs(entity.id);
      timeSectionOrder = await getTimeSectionOrder();
    } catch (e) {
      console.error('[entity] load failed:', e);
      viewNotes = [];
      viewFreeNotes = [];
      viewMarkers = [];
      incomingRefsRaw = [];
      timeSectionOrder = [];
    }
    viewNotesLoading = false;

    // If a specific marker was requested, activate it now that data is loaded
    if (focusStateId != null && viewMarkers.some(m => m.id === focusStateId)) {
      activeMarkerId = focusStateId;
    }
  }

  async function reloadMarkers() {
    if (viewEntity) {
      viewMarkers = await getEntityMarkers(viewEntity.id);
      stateKeySuggestions = await getDistinctStateKeys();
      timeSectionOrder = await getTimeSectionOrder();
      resolveStateAtCursor();
    }
  }

  // --- Checkpoint editing (all local until Save) ---

  function handleAddFactRow() {
    editRows = [...editRows, {
      type: 'fact',
      key: '',
      value: '',
      valueId: null,
      resolvedValue: '',
      isSet: true,
      isNew: true,
    }];
  }

  function handleAddEntityRef() {
    addingEntityRef = true;
  }

  function handleConfirmEntityRefLocal(refEntityId) {
    const entity = entities.find(e => e.id === refEntityId);
    editRows = [...editRows, {
      type: 'entity_ref',
      valueId: null,
      refEntityId,
      refEntityName: entity?.name || 'Unknown',
      refActive: true,
      isSet: true,
      isNew: true,
    }];
    addingEntityRef = false;
  }

  function handleRemoveEditRow(index) {
    editRows = editRows.filter((_, i) => i !== index);
  }

  async function handleSaveCheckpoint() {
    if (!activeMarkerId || !viewEntity) return;
    const marker = viewMarkers.find(m => m.id === activeMarkerId);
    if (!marker) return;

    // Delete all existing values for this marker, then re-create from editRows
    for (const existing of marker.values) {
      await deleteStateMarkerValue(existing.id);
    }

    // Save only rows marked as set by this checkpoint
    for (const row of editRows) {
      if (!row.isSet) continue;
      if (row.type === 'fact') {
        if (!row.key && !row.value) continue;
        await addStateMarkerValue(activeMarkerId, row.key, row.value);
      } else if (row.type === 'entity_ref' && row.refEntityId) {
        await addStateMarkerEntityRef(activeMarkerId, row.refEntityId, row.refActive);
      }
    }

    activeMarkerId = null;
    await reloadMarkers();
  }

  async function handleDeleteMarker(markerId) {
    await deleteStateMarker(markerId);
    ondeletestate?.(markerId);
    activeMarkerId = null;
    await reloadMarkers();
  }

  function getChapterName(chapterId) {
    const ch = chapters.find(c => c.id === chapterId);
    return ch ? ch.title : `Chapter ${chapterId}`;
  }

  async function pinSelectedText(entityId) {
    if (!selectedText || !selectedText.trim()) return;
    const chapterId = activeChapterId || null;
    const entity = entities.find(e => e.id === entityId);
    await addEntityNote(entityId, chapterId, selectedText.trim());
    if (viewEntity && viewEntity.id === entityId) {
      viewNotes = await getEntityNotes(entityId);
    }
    addToast(`Pinned to ${entity?.name || 'entity'}`, 'success');
    onclearselection?.();
  }

  async function removeNote(noteId) {
    if (!viewEntity) return;
    await deleteEntityNote(noteId);
    viewNotes = await getEntityNotes(viewEntity.id);
  }

  function handleExcerptDndConsider(e) {
    viewNotes = e.detail.items;
  }

  async function handleExcerptDndFinalize(e) {
    viewNotes = e.detail.items;
    await reorderEntityNotes(viewNotes.map(n => n.id));
  }

  async function addFreeNote() {
    if (!viewEntity || !newNoteText.trim()) return;
    await addEntityFreeNote(viewEntity.id, newNoteText.trim());
    newNoteText = '';
    viewFreeNotes = await getEntityFreeNotes(viewEntity.id);
  }

  async function saveFreeNote(note) {
    await updateEntityFreeNote(note.id, note.text);
  }

  async function removeFreeNote(id) {
    if (!viewEntity) return;
    await deleteEntityFreeNote(id);
    viewFreeNotes = await getEntityFreeNotes(viewEntity.id);
  }

  function handleFreeNoteDndConsider(e) {
    viewFreeNotes = e.detail.items;
  }

  async function handleFreeNoteDndFinalize(e) {
    viewFreeNotes = e.detail.items;
    await reorderEntityFreeNotes(viewFreeNotes.map(n => n.id));
  }

  async function showReferences(entity) {
    refsEntity = entity;
    refsLoading = true;
    view = 'references';
    try {
      refsData = await findReferences(entity.id);
    } catch (e) {
      console.warn('Find references failed:', e);
      refsData = null;
    }
    refsLoading = false;
  }

  // Build highlighted HTML from context + highlights array
  function buildHighlightedSnippet(context, highlights, color) {
    if (!highlights || highlights.length === 0) return escapeHtml(context);

    // Sort highlights by offset
    const sorted = [...highlights].sort((a, b) => a.offset - b.offset);
    let result = '';
    let lastEnd = 0;

    for (const h of sorted) {
      // Text before this highlight
      if (h.offset > lastEnd) {
        result += escapeHtml(context.slice(lastEnd, h.offset));
      }
      // The highlighted text
      const end = h.offset + h.length;
      result += `<mark class="refs-highlight" style="background: ${color}30; color: ${color};">${escapeHtml(context.slice(h.offset, end))}</mark>`;
      lastEnd = end;
    }

    // Remaining text after last highlight
    if (lastEnd < context.length) {
      result += escapeHtml(context.slice(lastEnd));
    }

    return result;
  }

  function escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function highlightDetectWord(context, word) {
    if (!context || !word) return escapeHtml(context || '');
    const escaped = escapeHtml(context);
    const wordEscaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(`(\\b${wordEscaped}\\b)`, 'gi');
    return escaped.replace(re, '<strong>$1</strong>');
  }

  async function runDetection() {
    detecting = true;
    try {
      candidates = await detectEntities(3);
      // Sort by occurrence count, most common first
      candidates.sort((a, b) => b.count - a.count || a.text.localeCompare(b.text));
    } catch (e) {
      console.warn('Detection failed:', e);
      candidates = [];
    }
    detecting = false;
    browsingCandidate = null;
  }

  async function acceptCandidate(candidate, type) {
    await oncreate(candidate.text, type, '', defaultColors[type]);
    candidates = candidates.filter(c => c.text !== candidate.text);
    if (browsingCandidate === candidate.text) browsingCandidate = null;
  }

  async function dismissCandidate(candidate) {
    await addIgnoredWord(candidate.text);
    candidates = candidates.filter(c => c.text !== candidate.text);
    if (browsingCandidate === candidate.text) browsingCandidate = null;
  }

  async function linkAsAlias(candidate, entityId) {
    await onaliasadd(entityId, candidate.text);
    candidates = candidates.filter(c => c.text !== candidate.text);
    linkingCandidate = null;
  }

  function browseCandidate(candidate) {
    if (browsingCandidate === candidate.text) {
      // Cycle to next location
      browseIndex = (browseIndex + 1) % candidate.locations.length;
    } else {
      browsingCandidate = candidate.text;
      browseIndex = 0;
    }
    const loc = candidate.locations[browseIndex];
    if (loc && ongotochapter) {
      ongotochapter(loc.chapter_id, candidate.text, loc.context);
    }
  }

  function startCreate() {
    newName = selectedText || '';
    newType = 'character';
    newDescription = '';
    newColor = defaultColors['character'];
    view = 'create';
  }

  async function submitCreate() {
    if (!newName.trim()) return;
    await oncreate(newName.trim(), newType, newDescription.trim(), newColor);
    view = 'list';
  }

  function startEdit(entity) {
    editingEntity = entity;
    editName = entity.name;
    editType = entity.entity_type;
    editDescription = entity.description || '';
    editColor = entity.color || defaultColors[entity.entity_type];
    newAlias = '';
    view = 'edit';
  }

  async function submitEdit() {
    if (!editName.trim() || !editingEntity) return;
    await onupdate(editingEntity.id, editName.trim(), editType, editDescription.trim(), editColor);
    editingEntity = { ...editingEntity, name: editName.trim(), entity_type: editType, description: editDescription.trim(), color: editColor };
  }

  async function handleAddAlias() {
    if (!newAlias.trim() || !editingEntity) return;
    await onaliasadd(editingEntity.id, newAlias.trim());
    editingEntity = { ...editingEntity, aliases: [...editingEntity.aliases, newAlias.trim()] };
    newAlias = '';
  }

  async function handleRemoveAlias(alias) {
    if (!editingEntity) return;
    await onaliasremove(editingEntity.id, alias);
    editingEntity = { ...editingEntity, aliases: editingEntity.aliases.filter(a => a !== alias) };
  }

  let showDeleteConfirm = $state(false);

  function handleDelete() {
    if (!editingEntity) return;
    showDeleteConfirm = true;
  }

  async function confirmDeleteEntity() {
    if (!editingEntity) return;
    showDeleteConfirm = false;
    await ondelete(editingEntity.id);
    editingEntity = null;
    view = 'list';
  }

  function cancelDeleteEntity() {
    showDeleteConfirm = false;
  }

  let filtered = $derived(() => {
    let list = entities;
    if (filterType !== 'all') {
      list = list.filter(e => e.entity_type === filterType);
    }
    if (filterText.trim()) {
      const q = filterText.toLowerCase();
      list = list.filter(e =>
        e.name.toLowerCase().includes(q) ||
        e.aliases.some(a => a.toLowerCase().includes(q))
      );
    }
    return list;
  });

  // Group by type
  let grouped = $derived(() => {
    const f = filtered();
    const groups = { character: [], place: [], thing: [] };
    for (const e of f) {
      if (groups[e.entity_type]) groups[e.entity_type].push(e);
    }
    return groups;
  });

  const typeLabels = { character: 'Characters', place: 'Places', thing: 'Things' };
</script>

<div class="entity-panel">
  {#if view === 'list'}

    {#if selectedText}
      <div class="selection-hint">
        Selected: <strong>{selectedText.length > 30 ? selectedText.slice(0, 30) + '...' : selectedText}</strong>
      </div>
    {/if}

    <div class="panel-filters">
      <div style="display: flex; gap: 2px; margin-bottom:5px;">
        <input class="filter-input" bind:value={filterText} placeholder="Filter entities..." />
        <button
                class="detect-btn"
                onclick={() => { view = 'detect'; runDetection(); }}
                title="Scan manuscript for entity candidates"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          Detect
        </button>
        <button
                class="add-btn"
                class:has-selection={selectedText}
                onclick={startCreate}
                title={selectedText ? `Create entity from "${selectedText}"` : 'Create new entity'}
        >
          + {selectedText ? 'Add' : 'New'}
        </button>
      </div>


      <div class="type-tabs">
        <button class="type-tab" class:active={filterType === 'all'} onclick={() => filterType = 'all'}>All</button>
        <button class="type-tab" class:active={filterType === 'character'} onclick={() => filterType = 'character'}>Characters</button>
        <button class="type-tab" class:active={filterType === 'place'} onclick={() => filterType = 'place'}>Places</button>
        <button class="type-tab" class:active={filterType === 'thing'} onclick={() => filterType = 'thing'}>Things</button>
      </div>
    </div>

    <div class="entity-list">
      {#each Object.entries(grouped()) as [type, items] (type)}
        {#if items.length > 0}
          <div class="entity-group">
            <div class="group-header">
              <span class="group-dot" style="background: {defaultColors[type]}"></span>
              <span class="group-label">{typeLabels[type]}</span>
              <span class="group-count">{items.length}</span>
            </div>
            {#each items as entity (entity.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="entity-item" class:viewed={entity.visible} onclick={() => showEntityView(entity)}>
                <span class="entity-color-dot" style="background: {entity.color}"></span>
                <div class="entity-info">
                  <span class="entity-name">{entity.name}</span>
                  {#if entity.aliases.length > 0}
                    <span class="entity-aliases">
                      {entity.aliases.join(', ')}
                    </span>
                  {/if}
                </div>
                <button
                  class="entity-action-btn"
                  class:active={entity.visible}
                  onclick={(e) => { e.stopPropagation(); ontoggleview?.(entity.id); }}
                  title={entity.visible ? 'Hide highlights' : 'Show highlights'}
                >
                  <i class="bi" class:bi-eye-fill={entity.visible} class:bi-eye-slash={!entity.visible}></i>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      {/each}

      {#if entities.length === 0}
        <div class="panel-empty">
          <p class="empty-title">No entities yet</p>
          <p class="empty-hint">
            {#if selectedText}
              Click <strong>+ Add</strong> above to create one from your selection.
            {:else}
              Highlight text in the editor, then click <strong>+ Add</strong> to create an entity.
            {/if}
          </p>
        </div>
      {/if}
    </div>

  {:else if view === 'create'}
    <!-- Create View -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">New Entity</span>
    </div>

    <div class="entity-form">
      <label class="form-label">
        Name
        <input class="form-input" bind:value={newName} placeholder="Entity name..." />
      </label>

      <label class="form-label">
        Type
        <div class="type-select">
          <button class="type-option" class:selected={newType === 'character'} onclick={() => { newType = 'character'; newColor = defaultColors.character; }}>
            <span class="type-dot" style="background: {defaultColors.character}"></span> Character
          </button>
          <button class="type-option" class:selected={newType === 'place'} onclick={() => { newType = 'place'; newColor = defaultColors.place; }}>
            <span class="type-dot" style="background: {defaultColors.place}"></span> Place
          </button>
          <button class="type-option" class:selected={newType === 'thing'} onclick={() => { newType = 'thing'; newColor = defaultColors.thing; }}>
            <span class="type-dot" style="background: {defaultColors.thing}"></span> Thing
          </button>
        </div>
      </label>

      <label class="form-label">
        Color
        <div class="color-pick">
          <input type="color" class="color-input" bind:value={newColor} />
          <span class="color-hex">{newColor}</span>
        </div>
      </label>

      <label class="form-label">
        Description
        <textarea class="form-textarea" bind:value={newDescription} placeholder="Optional notes..." rows="3"></textarea>
      </label>

      <div class="form-actions">
        <button class="btn-author btn-author-primary" onclick={submitCreate}>Create Entity</button>
        <button class="btn-author btn-author-subtle" onclick={() => view = 'list'}>Cancel</button>
      </div>
    </div>

  {:else if view === 'detect'}
    <!-- Detection View -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">Detect Entities</span>
      <button class="detect-btn" onclick={runDetection} title="Re-scan">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6"/><path d="M20.49 15a9 9 0 11-2.12-9.36L23 10"/></svg>
      </button>
    </div>

    <div class="detect-list">
      {#if detecting}
        <div class="detect-loading">Scanning manuscript...</div>
      {:else if candidates.length === 0}
        <div class="detect-empty">No new entity candidates found.</div>
      {:else}
        <div class="detect-count">{candidates.length} candidate{candidates.length !== 1 ? 's' : ''} found</div>
        {#each candidates as candidate (candidate.text)}
          <div class="detect-item" class:browsing={browsingCandidate === candidate.text}>
            <div class="detect-item-header">
              <span class="detect-name">{candidate.text}</span>
              <div class="detect-header-right">
                <span class="detect-freq">{candidate.count}&times;</span>
                <button
                  class="detect-goto"
                  class:active={browsingCandidate === candidate.text}
                  onclick={() => browseCandidate(candidate)}
                  title="Go to in text (click again for next)"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                  {#if browsingCandidate === candidate.text}
                    {browseIndex + 1}/{candidate.locations.length}
                  {/if}
                </button>
              </div>
            </div>
            {#if candidate.locations.length > 0}
              {#if candidate.locations.length > 0}
              <div class="detect-context">
                {#if browsingCandidate === candidate.text}
                  <span class="detect-chapter-badge">{candidate.locations[browseIndex]?.chapter_title}</span>
                {/if}
                &ldquo;...{@html highlightDetectWord(
                  browsingCandidate === candidate.text ? candidate.locations[browseIndex]?.context : candidate.locations[0]?.context,
                  candidate.text
                )}...&rdquo;
              </div>
            {/if}
            {/if}
            <div class="detect-actions">
              <button class="detect-type-btn" onclick={() => acceptCandidate(candidate, 'character')} title="Add as character">
                <span class="type-dot" style="background: {defaultColors.character}"></span> Character
              </button>
              <button class="detect-type-btn" onclick={() => acceptCandidate(candidate, 'place')} title="Add as place">
                <span class="type-dot" style="background: {defaultColors.place}"></span> Place
              </button>
              <button class="detect-type-btn" onclick={() => acceptCandidate(candidate, 'thing')} title="Add as thing">
                <span class="type-dot" style="background: {defaultColors.thing}"></span> Thing
              </button>
              <button class="detect-type-btn" onclick={() => linkingCandidate = linkingCandidate === candidate.text ? null : candidate.text} title="Link as alias to existing entity">
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/></svg>
                Alias
              </button>
              <button class="detect-dismiss" onclick={() => dismissCandidate(candidate)}>
                Ignore
              </button>
            </div>
            {#if linkingCandidate === candidate.text}
              <div class="link-picker">
                <span class="link-label">Link "{candidate.text}" as alias of:</span>
                {#each entities as entity (entity.id)}
                  <button class="link-entity" onclick={() => linkAsAlias(candidate, entity.id)}>
                    <span class="type-dot" style="background: {entity.color}"></span>
                    {entity.name}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>

  {:else if view === 'references'}
    <!-- References View -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">List</span>
    </div>

    {#if refsEntity}
      <div class="refs-entity-header">
        <span class="refs-color" style="background: {refsEntity.color}"></span>
        <span class="refs-name">List</span>
        {#if refsData}
          <span class="refs-total">{refsData.total} mention{refsData.total !== 1 ? 's' : ''}</span>
        {/if}
      </div>
    {/if}

    <div class="refs-list">
      {#if refsLoading}
        <div class="refs-loading">Scanning manuscript...</div>
      {:else if !refsData || refsData.total === 0}
        <div class="refs-empty">No references found in the manuscript.</div>
      {:else}
        {#each refsData.by_chapter as chapter (chapter.chapter_id)}
          <div class="refs-chapter">
            <div class="refs-chapter-header">
              <span class="refs-chapter-title">{chapter.chapter_title}</span>
              <span class="refs-chapter-count">{chapter.references.length}</span>
            </div>
            {#each chapter.references as ref, i}
              <button
                class="refs-snippet"
                onclick={() => ongotochapter?.(chapter.chapter_id, ref.matched_text, ref.anchor)}
                title="Jump to this reference"
              >
                <span class="refs-snippet-text">
                  &ldquo;...{@html buildHighlightedSnippet(ref.context, ref.highlights, refsEntity.color)}...&rdquo;
                </span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

  {:else if view === 'view'}
    <!-- Combined Entity View + Edit -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">List</span>
      <div class="view-header-actions">
        <button class="entity-action-btn" onclick={() => showReferences(viewEntity)} title="Find references">
          <i class="bi bi-search" style="font-size: 0.85rem;"></i>
        </button>
      </div>
    </div>

    {#if viewEntity}
      <!-- Inline edit: name + type + color -->
      <div class="view-edit-section">
        <div class="view-edit-row">
          <input type="color" class="view-color-input" bind:value={editColor} onchange={submitEdit} title="Entity color" />
          <input class="view-name-input" bind:value={editName} onblur={submitEdit} onkeydown={e => { if (e.key === 'Enter') e.target.blur(); }} placeholder="Name..." />
        </div>
        <div class="type-select">
          <button class="type-option" class:selected={editType === 'character'} onclick={() => { editType = 'character'; submitEdit(); }}>
            <span class="type-dot" style="background: {defaultColors.character}"></span> Character
          </button>
          <button class="type-option" class:selected={editType === 'place'} onclick={() => { editType = 'place'; submitEdit(); }}>
            <span class="type-dot" style="background: {defaultColors.place}"></span> Place
          </button>
          <button class="type-option" class:selected={editType === 'thing'} onclick={() => { editType = 'thing'; submitEdit(); }}>
            <span class="type-dot" style="background: {defaultColors.thing}"></span> Thing
          </button>
        </div>
      </div>

      <!-- Aliases -->
      <div class="view-aliases-section">
        <div class="alias-list">
          {#each editingEntity?.aliases || [] as alias}
            <div class="alias-tag">
              <span>{alias}</span>
              <button class="alias-remove" onclick={() => handleRemoveAlias(alias)}>&times;</button>
            </div>
          {/each}
        </div>
        <form class="alias-add" onsubmit={e => { e.preventDefault(); handleAddAlias(); }}>
          <input class="form-input alias-input" bind:value={newAlias} placeholder="Add alias..." />
          <button class="btn-author btn-author-subtle btn-author-sm" type="submit">Add</button>
        </form>
      </div>

      <!-- Pin selected text from editor -->
      {#if selectedText}
        <div class="view-pin-bar">
          <span class="view-pin-preview">&ldquo;{selectedText.length > 60 ? selectedText.slice(0, 60) + '...' : selectedText}&rdquo;</span>
          <button class="view-pin-btn" onclick={() => pinSelectedText(viewEntity.id)}>
            <i class="bi bi-pin-angle"></i> Pin excerpt
          </button>
        </div>
      {/if}

      <!-- Tabs: Excerpts / Notes -->
      <div class="view-tabs">
        <button class="view-tab" class:active={viewTab === 'excerpts'} onclick={() => viewTab = 'excerpts'}>
          <i class="bi bi-pin-angle"></i> Excerpts
          <span class="view-tab-count">{viewNotes.length}</span>
        </button>
        <button class="view-tab" class:active={viewTab === 'notes'} onclick={() => viewTab = 'notes'}>
          <i class="bi bi-journal-text"></i> Notes
          <span class="view-tab-count">{viewFreeNotes.length}</span>
        </button>
        <button class="view-tab" class:active={viewTab === 'state'} onclick={() => viewTab = 'state'}>
          <i class="bi bi-diamond"></i> State
          <span class="view-tab-count">{viewMarkers.length}</span>

        </button>
      </div>

      {#if viewTab === 'excerpts'}
        <div class="view-notes-list">
          {#if viewNotesLoading}
            <div class="view-notes-empty">Loading...</div>
          {:else if viewNotes.length === 0}
            <div class="view-notes-empty">
              No pinned excerpts yet. Highlight text in the editor and click
              <i class="bi bi-pin-angle"></i> to pin it here.
            </div>
          {:else}
            <div class="dnd-zone" use:dndzone={{ items: viewNotes, flipDurationMs: 200 }} onconsider={handleExcerptDndConsider} onfinalize={handleExcerptDndFinalize}>
              {#each viewNotes as note (note.id)}
                <div class="view-note">
                  <div class="view-note-drag"><i class="bi bi-grip-vertical"></i></div>
                  <div class="view-note-body">
                    <button class="view-note-text clickable" onclick={() => {
                      // Use first few words as search text, full excerpt as anchor
                      const words = note.text.split(/\s+/).slice(0, 5).join(' ');
                      ongotochapter?.(note.chapter_id, words, note.text.slice(0, 40));
                    }} title="Jump to this excerpt">
                      &ldquo;{note.text}&rdquo;
                    </button>
                    <div class="view-note-footer">
                      <span class="view-note-date">{new Date(note.created_at).toLocaleDateString()}</span>
                      <button class="view-note-delete" onclick={() => removeNote(note.id)} title="Remove">
                        <i class="bi bi-x"></i>
                      </button>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>

      {:else if viewTab === 'notes'}
        <div class="view-notes-list">
          <div class="free-note-add">
            <textarea class="free-note-input" bind:value={newNoteText} placeholder="Write a note..." rows="4"></textarea>
            <button class="view-pin-btn" onclick={addFreeNote} disabled={!newNoteText.trim()}>
              <i class="bi bi-plus"></i> Add
            </button>
          </div>

          {#if viewFreeNotes.length === 0}
            <div class="view-notes-empty">No notes yet. Write one above.</div>
          {:else}
            <div class="dnd-zone" use:dndzone={{ items: viewFreeNotes, flipDurationMs: 200 }} onconsider={handleFreeNoteDndConsider} onfinalize={handleFreeNoteDndFinalize}>
              {#each viewFreeNotes as note (note.id)}
                <div class="view-note free-note-card">
                  <div class="view-note-drag"><i class="bi bi-grip-vertical"></i></div>
                  <div class="view-note-body">
                    <textarea
                      class="free-note-edit"
                      value={note.text}
                      oninput={e => { note.text = e.target.value; }}
                      onblur={() => saveFreeNote(note)}
                      rows="2"
                    ></textarea>
                    <div class="view-note-footer">
                      <span class="view-note-date">{new Date(note.created_at).toLocaleDateString()}</span>
                      <button class="view-note-delete" onclick={() => removeFreeNote(note.id)} title="Delete">
                        <i class="bi bi-x"></i>
                      </button>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if viewTab === 'state'}
        <div class="view-notes-list">
          {#if activeMarkerId}
            <!-- Editing a checkpoint: shows ALL entity variables with resolved values -->
            {@const marker = viewMarkers.find(m => m.id === activeMarkerId)}
            {#if marker}
              <div class="state-marker-edit">
                <div class="state-marker-edit-label">
                  Editing checkpoint — {getChapterName(marker.chapter_id)}
                </div>

                {#each editRows as row, i}
                  <div class="state-edit-item">
                    <div class="state-edit-left">
                      {#if row.type === 'fact'}
                        {#if row.isNew}
                          <input
                            class="state-key-input"
                            bind:value={row.key}
                            placeholder="Variable name"
                            list="state-key-suggestions"
                          />
                        {:else}
                          <span class="state-edit-label">{row.key}</span>
                        {/if}
                      {:else if row.type === 'entity_ref'}
                        <span class="state-edit-label">
                          {row.refEntityName}
                          <i class="bi bi-link-45deg state-ref-icon"></i>
                        </span>
                      {/if}
                    </div>
                    <div class="state-edit-right">
                      {#if row.type === 'fact'}
                        <input
                          class="state-value-input"
                          bind:value={row.value}
                          oninput={() => { row.isSet = true; }}
                          placeholder={row.resolvedValue || 'Not set'}
                        />
                      {:else if row.type === 'entity_ref'}
                        <label class="state-ref-toggle">
                          <input type="checkbox" bind:checked={row.refActive}
                            onchange={() => { row.isSet = true; }}
                          />
                          <span class="state-ref-label">{row.refActive ? 'Active' : 'Inactive'}</span>
                        </label>
                      {/if}
                      <button class="state-action-btn danger" onclick={() => handleRemoveEditRow(i)} title="Remove">
                        <i class="bi bi-x-lg"></i>
                      </button>
                    </div>
                  </div>
                {/each}

                {#if addingEntityRef}
                  <div class="state-ref-add-row">
                    <i class="bi bi-link-45deg state-ref-icon"></i>
                    <select class="state-entity-select" onchange={e => { if (e.target.value) handleConfirmEntityRefLocal(parseInt(e.target.value)); }}>
                      <option value="">Select entity...</option>
                      {#each entities.filter(en => en.id !== viewEntity?.id) as en}
                        <option value={en.id}>{en.name}</option>
                      {/each}
                    </select>
                    <button class="state-action-btn" onclick={() => { addingEntityRef = false; }} title="Cancel">
                      <i class="bi bi-x-lg"></i>
                    </button>
                  </div>
                {/if}

                <div class="state-add-buttons">
                  <button class="state-add-val-btn" onclick={handleAddFactRow}>
                    <i class="bi bi-plus"></i> Add variable
                  </button>
                  <button class="state-add-val-btn" onclick={handleAddEntityRef}>
                    <i class="bi bi-link-45deg"></i> Add entity reference
                  </button>
                </div>

                <div class="state-marker-edit-footer">
                  <div class="state-footer-left">
                    <button class="state-done-btn" onclick={handleSaveCheckpoint}>
                      <i class="bi bi-check-lg"></i> Save
                    </button>
                    <button class="state-cancel-btn" onclick={() => { activeMarkerId = null; }}>
                      Cancel
                    </button>
                  </div>
                  {#if showDeleteMarkerConfirm}
                    <div class="delete-confirm">
                      <p class="delete-confirm-msg">Delete this checkpoint?</p>
                      <div class="delete-confirm-actions">
                        <button class="btn-danger" onclick={() => handleDeleteMarker(marker.id)}>Delete</button>
                        <button class="btn-author btn-author-subtle btn-author-sm" onclick={() => { showDeleteMarkerConfirm = false; }}>Cancel</button>
                      </div>
                    </div>
                  {:else}
                    <button class="btn-danger-subtle" onclick={() => { showDeleteMarkerConfirm = true; }}>
                      <i class="bi bi-trash"></i> Delete checkpoint
                    </button>
                  {/if}
                </div>
              </div>
            {/if}
          {:else}
            <div class="state-view-container">
              <!-- Sub-view toggle -->
              <div class="state-sub-tabs">
                <button class="state-sub-tab" class:active={stateSubView === 'cursor'} onclick={() => stateSubView = 'cursor'}>
                  <i class="bi bi-crosshair"></i> State at cursor
                </button>
                <button class="state-sub-tab" class:active={stateSubView === 'all'} onclick={() => stateSubView = 'all'}>
                  <i class="bi bi-list-ul"></i> All state changes
                </button>
              </div>

              {#if stateSubView === 'cursor'}
                <!-- Resolved state at cursor position -->
                {#if resolvedState?.facts?.length > 0 || resolvedState?.entityRefs?.length > 0}
                  <div class="state-filter-row">
                    <input class="state-filter-input" bind:value={stateFilterKey} placeholder="Filter name..." />
                    <input class="state-filter-input" bind:value={stateFilterValue} placeholder="Filter value..." />
                  </div>
                  <div class="state-resolved-list">
                    {#if resolvedState?.facts?.length > 0}
                      {#each resolvedState.facts.filter(f => {
                        const kMatch = !stateFilterKey || f.key.toLowerCase().includes(stateFilterKey.toLowerCase());
                        const vMatch = !stateFilterValue || f.value.toLowerCase().includes(stateFilterValue.toLowerCase());
                        return kMatch && vMatch;
                      }) as f}
                        <div class="state-resolved-row">
                          <div class="state-resolved-key">{f.key}</div>
                          <div class="state-resolved-value">{f.value}</div>
                        </div>
                      {/each}
                    {/if}
                    {#if resolvedState?.entityRefs?.length > 0}
                      {#each resolvedState.entityRefs.filter(r => {
                        const kMatch = !stateFilterKey || r.refEntityName.toLowerCase().includes(stateFilterKey.toLowerCase());
                        const vMatch = !stateFilterValue || (r.refActive ? 'active' : 'inactive').includes(stateFilterValue.toLowerCase());
                        return kMatch && vMatch;
                      }) as r}
                        <div class="state-resolved-row state-resolved-ref">
                          <div class="state-resolved-key">
                            <i class="bi bi-link-45deg" style="font-size: 0.75rem;"></i>
                            {r.refEntityName}
                          </div>
                          <div class="state-resolved-value" class:state-ref-inactive={!r.refActive}>
                            {r.refActive ? 'Active' : 'Inactive'}
                          </div>
                        </div>
                      {/each}
                    {/if}
                  </div>
                {:else if viewMarkers.length > 0}
                  <div class="view-notes-empty">
                    No state values at this position. Checkpoints exist but are placed after the cursor.
                  </div>
                {:else}
                  <div class="view-notes-empty">
                    No state set. Right-click an entity mention and select "Set state value" to place a checkpoint.
                  </div>
                {/if}

                {#if resolvedIncoming.length > 0}
                  <div class="state-incoming-section">
                    <div class="state-incoming-header">
                      <i class="bi bi-box-arrow-in-left"></i> Referenced by
                    </div>
                    {#each resolvedIncoming as ref}
                      <div class="state-incoming-row">
                        <div class="state-incoming-name">{ref.srcName}</div>
                        <div class="state-incoming-status" class:state-ref-inactive={!ref.active}>
                          {ref.active ? 'Active' : 'Inactive'}
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}

              {:else if stateSubView === 'all'}
                <!-- All state changes for this entity -->
                {#if viewMarkers.length === 0}
                  <div class="view-notes-empty">
                    No state checkpoints yet.
                  </div>
                {:else}
                  {#each viewMarkers as marker (marker.id)}
                    <div class="state-change-card" onclick={() => onnavtomarker?.(marker.id, marker.chapter_id)} role="button" tabindex="0">
                      <div class="state-change-header">
                        <span class="state-change-chapter">
                          <i class="bi bi-diamond" style="font-size: 0.65rem; color: #2d6a5e;"></i>
                          {getChapterName(marker.chapter_id)}
                        </span>
                        <button class="state-change-edit" onclick={e => { e.stopPropagation(); activeMarkerId = marker.id; }} title="Edit this checkpoint">
                          <i class="bi bi-pencil"></i>
                        </button>
                      </div>
                      {#if marker.values.length > 0}
                        <div class="state-change-values">
                          {#each marker.values as v}
                            {#if v.value_type === 'entity_ref'}
                              <div class="state-change-val">
                                <span class="state-change-key">
                                  {v.ref_entity_name || 'Unknown'}
                                  <i class="bi bi-link-45deg" style="font-size: 0.7rem; color: var(--iwe-text-faint);"></i>
                                </span>
                                <span class="state-change-value" class:state-ref-inactive={!v.ref_active}>
                                  {v.ref_active ? 'Active' : 'Inactive'}
                                </span>
                              </div>
                            {:else}
                              <div class="state-change-val">
                                <span class="state-change-key">{v.fact_key}</span>
                                <span class="state-change-value">{v.fact_value}</span>
                              </div>
                            {/if}
                          {/each}
                        </div>
                      {:else}
                        <div class="state-change-empty">No values set</div>
                      {/if}
                    </div>
                  {/each}
                {/if}
              {/if}
            </div>
          {/if}

          <datalist id="state-key-suggestions">
            {#each stateKeySuggestions as key}
              <option value={key} />
            {/each}
          </datalist>
        </div>
      {/if}

      <!-- Delete -->
      <div class="view-delete-section">
        {#if showDeleteConfirm}
          <div class="delete-confirm">
            <p class="delete-confirm-msg">Delete <strong>"{editingEntity?.name}"</strong>?</p>
            <div class="delete-confirm-actions">
              <button class="btn-danger" onclick={confirmDeleteEntity}>Delete</button>
              <button class="btn-author btn-author-subtle btn-author-sm" onclick={cancelDeleteEntity}>Cancel</button>
            </div>
          </div>
        {:else}
          <button class="btn-danger-subtle" onclick={handleDelete}>Delete Entity</button>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .entity-panel {
    display: flex; flex-direction: column; height: 100%;
    font-family: var(--iwe-font-ui);
    font-size: 0.9rem;
  }

  .panel-header {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.6rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .panel-label {
    font-size: 0.7rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.08em; color: var(--iwe-text-muted); flex: 1;
  }

  .add-btn {
    font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500;
    padding: 0.25rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: var(--iwe-bg); color: var(--iwe-text-secondary);
    transition: all 150ms;
  }
  .add-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .add-btn.has-selection {
    background: var(--iwe-accent); color: white; border-color: var(--iwe-accent);
  }
  .add-btn.has-selection:hover { background: var(--iwe-accent-hover); }

  .selection-hint {
    padding: 0.4rem 0.75rem; font-size: 0.7rem; color: var(--iwe-text-muted);
    background: var(--iwe-accent-light); border-bottom: 1px solid var(--iwe-border-light);
  }
  .selection-hint strong { color: var(--iwe-accent); font-weight: 600; }

  .back-btn {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-muted); font-size: 1rem; padding: 0.1rem 0.3rem;
    border-radius: var(--iwe-radius-sm);
  }
  .back-btn:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }

  /* Filters */
  .panel-filters {
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--iwe-border-light);

  }
  .panel-filters button {
    font-size:0.9rem;
  }
  .filter-input {
    /*width: 100%; */
    padding: 0.35rem 0.5rem;
    font-size: 0.9rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    font-family: var(--iwe-font-ui); outline: none;
    /*margin-bottom: 0.4rem;*/
  }
  .filter-input:focus { border-color: var(--iwe-accent); }
  .filter-input::placeholder { color: var(--iwe-text-faint); }

  .type-tabs { display: flex; gap: 2px; }
  .type-tab {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    padding: 0.2rem 0.45rem; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; background: none; color: var(--iwe-text-muted);
    transition: all 100ms;
  }
  .type-tab:hover { background: var(--iwe-bg-hover); }
  .type-tab.active { background: var(--iwe-accent-light); color: var(--iwe-accent); font-weight: 500; }

  /* Entity list */
  .entity-list { flex: 1; overflow-y: auto; padding: 0.25rem 0 3rem; }

  .entity-group { margin-bottom: 0.25rem; }
  .group-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.75rem;
  }
  .group-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .group-label {
    font-size: 0.8rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.06em; color: var(--iwe-text-faint);
  }
  .group-count { font-size: 0.7rem; color: var(--iwe-text-faint); }

  .entity-item {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%;
    padding: 0.45rem 0.5rem 0.45rem 1.5rem;
    transition: background 100ms;
    font-family: var(--iwe-font-ui);
    border-bottom: 1px solid var(--iwe-border-light);
    cursor: pointer;
  }
  .entity-item:hover { background: var(--iwe-bg-hover); }
  .entity-item:hover .entity-actions { opacity: 1; }
  .entity-item.viewed .entity-actions { opacity: 1; }

  .entity-color-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
  }
  .entity-info {
    display: flex; flex-direction: column; gap: 0.1rem; min-width: 0; flex: 1;
  }
  .entity-name {
    font-size: 0.9rem;
    color: var(--iwe-text);
  }
  .entity-aliases {
    font-size: 0.7rem; color: var(--iwe-text-faint); font-style: italic;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .entity-actions {
    display: flex; gap: 2px; flex-shrink: 0; opacity: 0;
    transition: opacity 100ms;
  }
  .entity-action-btn {
    background: none; border: 1px solid transparent;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text-faint); padding: 4px;
    display: flex; align-items: center; justify-content: center;
    transition: all 100ms; min-width: 26px; min-height: 26px;
  }
  .entity-action-btn:hover {
    background: var(--iwe-bg-active); color: var(--iwe-text);
    border-color: var(--iwe-border);
  }
  .entity-action-btn.active {
    color: var(--iwe-accent); background: var(--iwe-accent-light);
  }

  /* Empty state */
  .panel-empty { text-align: center; padding: 2rem 1rem; }
  .empty-title { font-size: 0.85rem; color: var(--iwe-text-muted); margin: 0 0 0.25rem; }
  .empty-hint { font-size: 0.75rem; color: var(--iwe-text-faint); margin: 0; line-height: 1.5; }
  .empty-hint strong { color: var(--iwe-accent); }

  /* Form */
  .entity-form { padding: 0.75rem; display: flex; flex-direction: column; gap: 0.75rem; }
  .form-label {
    display: flex; flex-direction: column; gap: 0.3rem;
    font-size: 0.7rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.06em; color: var(--iwe-text-muted);
  }
  .form-input {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    padding: 0.4rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none;
  }
  .form-input:focus { border-color: var(--iwe-accent); }
  .form-textarea {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    padding: 0.4rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none; resize: vertical;
  }
  .form-textarea:focus { border-color: var(--iwe-accent); }

  .type-select { display: flex; gap: 4px; }
  .type-option {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.35rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: var(--iwe-bg); color: var(--iwe-text-secondary);
    transition: all 100ms;
  }
  .type-option:hover { border-color: var(--iwe-text-faint); }
  .type-option.selected { background: var(--iwe-accent-light); color: var(--iwe-accent); border-color: var(--iwe-accent); }
  .type-dot { width: 7px; height: 7px; border-radius: 50%; }

  /* Aliases */
  .alias-list { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 0.3rem; }
  .alias-tag {
    display: inline-flex; align-items: center; gap: 0.2rem;
    padding: 0.15rem 0.4rem; background: var(--iwe-bg-hover);
    border-radius: var(--iwe-radius-sm); font-size: 0.8rem;
    color: var(--iwe-text-secondary);
  }
  .alias-remove {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.9rem; line-height: 1;
    padding: 0 2px;
  }
  .alias-remove:hover { color: var(--iwe-danger); }
  .alias-add { display: flex; gap: 0.3rem; }
  .alias-add .form-input { flex: 1; font-size: 0.8rem; padding: 0.3rem 0.5rem; }

  .form-actions { display: flex; gap: 0.5rem; padding-top: 0.5rem; }

  .btn-danger {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.35rem 0.8rem; border: 1px solid var(--iwe-danger);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-danger); transition: all 150ms;
  }
  .btn-danger:hover { background: var(--iwe-danger); color: white; }

  .delete-confirm {
    background: var(--iwe-danger-light); border: 1px solid var(--iwe-danger);
    border-radius: var(--iwe-radius); padding: 0.6rem 0.8rem;
    animation: deleteConfirmIn 0.15s ease;
  }
  @keyframes deleteConfirmIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .delete-confirm-msg {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text); margin: 0 0 0.5rem;
  }
  .delete-confirm-actions {
    display: flex; gap: 0.4rem;
  }

  /* Color picker */
  .color-pick {
    display: flex; align-items: center; gap: 0.5rem;
  }
  .color-input {
    width: 32px; height: 26px; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    padding: 1px; background: var(--iwe-bg);
  }
  .color-input::-webkit-color-swatch-wrapper { padding: 0; }
  .color-input::-webkit-color-swatch { border: none; border-radius: 2px; }
  .color-hex { font-size: 0.75rem; color: var(--iwe-text-faint); }
  .btn-text-sm {
    background: none; border: none; font-family: var(--iwe-font-ui);
    font-size: 0.7rem; color: var(--iwe-text-faint); cursor: pointer;
    padding: 0;
  }
  .btn-text-sm:hover { color: var(--iwe-accent); }

  /* Detect button */
  .detect-btn {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    padding: 0.2rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: inline-flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .detect-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  /* Detection view */
  .detect-list { flex: 1; overflow-y: auto; padding: 0.25rem 0; }
  .detect-loading, .detect-empty {
    text-align: center; padding: 2rem 1rem;
    font-size: 0.85rem; color: var(--iwe-text-muted); font-style: italic;
  }
  .detect-count {
    padding: 0.3rem 0.75rem; font-size: 0.7rem; color: var(--iwe-text-faint);
    font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em;
  }

  .detect-item {
    padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .detect-item:hover { background: var(--iwe-bg-hover); }
  .detect-item.browsing { background: var(--iwe-accent-light); }
  .detect-item-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 0.2rem;
  }
  .detect-header-right {
    display: flex; align-items: center; gap: 0.4rem;
  }
  .detect-name {
    font-family: var(--iwe-font-prose); font-size: 0.9rem;
    font-weight: 500; color: var(--iwe-text);
  }
  .detect-freq { font-size: 0.7rem; color: var(--iwe-text-faint); }
  .detect-goto {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    padding: 0.15rem 0.35rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: inline-flex; align-items: center; gap: 0.2rem;
    transition: all 100ms;
  }
  .detect-goto:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .detect-goto.active { background: var(--iwe-accent-light); color: var(--iwe-accent); border-color: var(--iwe-accent); }
  .detect-context {
    font-size: 0.75rem; color: var(--iwe-text-muted); font-style: italic;
    margin-bottom: 0.4rem; line-height: 1.5;
  }
  .detect-chapter-badge {
    font-style: normal; font-size: 0.65rem; font-weight: 500;
    background: var(--iwe-bg-active); color: var(--iwe-text-secondary);
    padding: 0.1rem 0.3rem; border-radius: 2px; margin-right: 0.3rem;
  }
  .detect-actions { display: flex; gap: 0.25rem; flex-wrap: wrap; }
  .detect-type-btn {
    font-family: var(--iwe-font-ui); font-size: 0.65rem; font-weight: 500;
    padding: 0.15rem 0.4rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-secondary);
    display: inline-flex; align-items: center; gap: 0.2rem;
    transition: all 100ms;
  }
  .detect-type-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); background: var(--iwe-accent-light); }
  .detect-dismiss {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    padding: 0.15rem 0.4rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-faint);
    transition: all 100ms;
  }
  .detect-dismiss:hover { border-color: var(--iwe-danger); color: var(--iwe-danger); }

  .link-picker {
    margin-top: 0.4rem; padding: 0.4rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
  }
  .link-label {
    display: block; font-size: 0.7rem; color: var(--iwe-text-muted);
    margin-bottom: 0.3rem;
  }
  .link-entity {
    display: flex; align-items: center; gap: 0.3rem; width: 100%;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.25rem 0.4rem; border: none; border-radius: var(--iwe-radius-sm);
    background: none; color: var(--iwe-text); cursor: pointer;
    text-align: left; transition: background 100ms;
  }
  .link-entity:hover { background: var(--iwe-accent-light); color: var(--iwe-accent); }

  /* References view */
  .refs-entity-header {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.6rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
    background: var(--iwe-bg-hover);
  }
  .refs-color { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .refs-name {
    font-family: var(--iwe-font-prose); font-size: 0.95rem;
    font-weight: 500; color: var(--iwe-text); flex: 1;
  }
  .refs-total {
    font-size: 0.7rem; color: var(--iwe-text-faint);
  }

  .refs-list { flex: 1; overflow-y: auto; }
  .refs-loading, .refs-empty {
    text-align: center; padding: 2rem 1rem;
    font-size: 0.85rem; color: var(--iwe-text-muted); font-style: italic;
  }

  .refs-chapter { border-bottom: 1px solid var(--iwe-border); }
  .refs-chapter-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem; position: sticky; top: 0; z-index: 1;
    background: var(--iwe-bg-sidebar);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .refs-chapter-title {
    font-family: var(--iwe-font-prose);
    font-size: 1rem; font-weight: 600; color: var(--iwe-text);
  }
  .refs-chapter-count {
    font-size: 0.75rem; color: var(--iwe-text-muted);
    background: var(--iwe-bg-active); padding: 0.15rem 0.5rem;
    border-radius: 10px; font-weight: 500;
  }

  .refs-snippet {
    display: block; width: 100%;
    background: none; border: none; border-bottom: 1px solid var(--iwe-border-light);
    padding: 0.75rem; cursor: pointer; text-align: left;
    transition: background 100ms;
  }
  .refs-snippet:last-child { border-bottom: none; }
  .refs-snippet:hover { background: var(--iwe-bg-hover); }

  .refs-snippet-text {
    font-family: var(--iwe-font-prose);
    font-size: 0.9rem; color: var(--iwe-text-secondary);
    line-height: 1.7;
    display: block;
  }

  .refs-snippet-text :global(.refs-highlight) {
    font-weight: 700; color: var(--iwe-text);
    border-radius: 2px; padding: 0 2px;
  }

  /* Entity view pane */
  .view-header-actions { display: flex; gap: 2px; margin-left: auto; }

  .view-edit-section {
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-edit-row {
    display: flex; align-items: center; gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  .view-color-input {
    width: 28px; height: 28px; border: 1px solid var(--iwe-border);
    border-radius: 50%; cursor: pointer; padding: 0;
    background: none; flex-shrink: 0;
  }
  .view-color-input::-webkit-color-swatch-wrapper { padding: 2px; }
  .view-color-input::-webkit-color-swatch { border: none; border-radius: 50%; }
  .view-name-input {
    flex: 1; font-family: var(--iwe-font-prose); font-size: 1.1rem;
    font-weight: 600; color: var(--iwe-text);
    border: none; background: none; outline: none;
    border-bottom: 1px dashed transparent; padding: 0.1rem 0;
  }
  .view-name-input:focus { border-bottom-color: var(--iwe-accent); }

  .view-aliases-section {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .alias-input { font-size: 0.8rem; padding: 0.25rem 0.5rem; }

  .view-delete-section {
    padding: 0.75rem;
    margin-top: auto;
    border-top: 1px solid var(--iwe-border-light);
  }
  .btn-danger-subtle {
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    padding: 0.3rem 0.6rem; border: none;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-faint); transition: all 150ms;
  }
  .btn-danger-subtle:hover { color: var(--iwe-danger); background: var(--iwe-danger-light); }

  .view-entity-header {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-color { width: 14px; height: 14px; border-radius: 50%; flex-shrink: 0; }
  .view-entity-info { display: flex; flex-direction: column; }
  .view-name {
    font-family: var(--iwe-font-prose); font-size: 1.1rem;
    font-weight: 600; color: var(--iwe-text);
  }
  .view-type {
    font-size: 0.7rem; color: var(--iwe-text-faint);
    text-transform: capitalize;
  }

  .view-aliases {
    padding: 0.5rem 0.75rem; display: flex; flex-wrap: wrap; gap: 0.3rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-alias-tag {
    font-size: 0.75rem; color: var(--iwe-text-secondary);
    background: var(--iwe-bg-hover); padding: 0.15rem 0.45rem;
    border-radius: 10px;
  }

  .view-description {
    padding: 0.6rem 0.75rem; font-size: 0.85rem;
    color: var(--iwe-text-secondary); line-height: 1.5;
    border-bottom: 1px solid var(--iwe-border-light);
    font-family: var(--iwe-font-prose);
  }

  .view-pin-bar {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: var(--iwe-accent-light);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-pin-preview {
    flex: 1; font-size: 0.8rem; color: var(--iwe-text-secondary);
    font-style: italic; overflow: hidden; text-overflow: ellipsis;
    white-space: nowrap;
  }
  .view-pin-btn {
    font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500;
    padding: 0.25rem 0.6rem; border: 1px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: var(--iwe-accent); color: white;
    display: inline-flex; align-items: center; gap: 0.25rem;
    transition: all 100ms; white-space: nowrap;
  }
  .view-pin-btn:hover { background: var(--iwe-accent-hover); }

  .view-section-header {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.5rem 0.75rem; font-size: 0.7rem; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-sidebar);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-section-count {
    font-size: 0.6rem; background: var(--iwe-bg-active);
    padding: 0.1rem 0.35rem; border-radius: 8px;
    color: var(--iwe-text-faint);
  }

  .view-notes-list { flex: 1; overflow-y: auto; }
  .view-notes-empty {
    padding: 1.5rem 0.75rem; text-align: center;
    font-size: 0.8rem; color: var(--iwe-text-faint); line-height: 1.6;
  }

  .view-note {
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-note:hover { background: var(--iwe-bg-hover); }
  .view-note-text {
    font-family: var(--iwe-font-prose); font-size: 0.85rem;
    color: var(--iwe-text-secondary); line-height: 1.6;
    background: none; border: none; padding: 0; margin: 0;
    text-align: left; width: 100%;
  }
  .view-note-text.clickable {
    cursor: pointer; transition: color 100ms;
  }
  .view-note-text.clickable:hover { color: var(--iwe-accent); }
  .view-note-footer {
    display: flex; align-items: center; justify-content: space-between;
    margin-top: 0.3rem;
  }
  .view-note-date { font-size: 0.65rem; color: var(--iwe-text-faint); }
  .view-note-delete {
    background: rgba(184, 84, 80, 0.08); border: 1px solid rgba(184, 84, 80, 0.2);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-danger); font-size: 1rem; padding: 0.15rem 0.4rem;
    transition: all 100ms; display: flex; align-items: center;
  }
  .view-note-delete:hover { background: var(--iwe-danger); color: white; border-color: var(--iwe-danger); }

  .entity-action-btn.has-selection {
    color: var(--iwe-accent);
  }
  .entity-action-btn.pin-btn {
    color: var(--iwe-accent); opacity: 1;
  }
  .entity-action-btn.pin-btn:hover {
    background: var(--iwe-accent); color: white;
  }

  /* View tabs */
  .view-tabs {
    display: flex; border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-tab {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500;
    background: none; border: none; border-bottom: 2px solid transparent;
    color: var(--iwe-text-muted); cursor: pointer; transition: all 150ms;
  }
  .view-tab:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-hover); }
  .view-tab.active { color: var(--iwe-text); border-bottom-color: var(--iwe-accent); }
  .view-tab-count {
    font-size: 0.6rem; background: var(--iwe-bg-active);
    padding: 0.05rem 0.3rem; border-radius: 8px; color: var(--iwe-text-faint);
  }

  /* Drag and drop */
  .dnd-zone { min-height: 20px; }
  .view-note {
    display: flex; align-items: flex-start; gap: 0.3rem;
    padding: 0.5rem 0.5rem 0.5rem 0.25rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .view-note:hover { background: var(--iwe-bg-hover); }
  .view-note-drag {
    color: var(--iwe-text-faint); cursor: grab;
    padding: 0.2rem 0; font-size: 0.9rem; flex-shrink: 0;
    opacity: 0.4;
  }
  .view-note-drag:hover { opacity: 1; }
  .view-note-body { flex: 1; min-width: 0; }

  /* Free notes */
  .free-note-add {
    padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
    display: flex; flex-direction: column; gap: 0.3rem;
  }
  .free-note-input {
    font-family: var(--iwe-font-prose); font-size: 0.95rem;
    padding: 0.5rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none; resize: vertical;
    line-height: 1.6;
  }
  .free-note-input:focus { border-color: var(--iwe-accent); }
  .free-note-input::placeholder { color: var(--iwe-text-faint); }
  .free-note-edit {
    width: 100%; font-family: var(--iwe-font-prose); font-size: 0.85rem;
    padding: 0.3rem 0.4rem; border: 1px solid transparent;
    border-radius: var(--iwe-radius-sm); background: transparent;
    color: var(--iwe-text-secondary); outline: none; resize: vertical;
    line-height: 1.6;
  }
  .free-note-edit:focus { border-color: var(--iwe-accent); background: var(--iwe-bg); }
  .free-note-card { }

  /* State tab */
  .state-entry {
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--iwe-border-subtle, #eee);
    position: relative;
  }
  .state-entry:hover .state-entry-actions { opacity: 1; }
  .state-entry-header {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    margin-bottom: 0.25rem;
  }
  .state-icon { font-size: 0.7rem; }
  .fact-icon { color: #2d6a5e; }
  .rel-icon { color: #6a4c2d; }
  .state-type-label {
    font-weight: 600;
    color: var(--iwe-text-secondary);
    text-transform: uppercase;
    font-size: 0.65rem;
    letter-spacing: 0.05em;
  }
  .state-chapter-ref {
    margin-left: auto;
    font-size: 0.7rem;
    color: var(--iwe-text-faint);
  }
  .state-rel-entity {
    font-weight: 600;
    color: var(--iwe-text);
  }
  .state-key {
    font-weight: 600;
    color: var(--iwe-text);
    font-family: var(--iwe-font-ui);
  }
  .state-eq {
    color: var(--iwe-text-faint);
    font-size: 0.8rem;
  }
  .state-value {
    color: var(--iwe-text-secondary);
    font-family: var(--iwe-font-prose);
  }
  .state-note {
    font-size: 0.78rem;
    color: var(--iwe-text-faint);
    font-style: italic;
    padding: 0.1rem 0 0.15rem;
  }
  .state-entry-actions {
    position: absolute;
    top: 0.4rem;
    right: 0.4rem;
    display: flex;
    gap: 0.2rem;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .state-action-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.15rem 0.3rem;
    border-radius: var(--iwe-radius-sm);
    color: var(--iwe-text-faint);
    font-size: 0.75rem;
  }
  .state-action-btn:hover { background: var(--iwe-hover); color: var(--iwe-text); }
  .state-action-btn.danger:hover { color: #c0392b; background: rgba(192, 57, 43, 0.08); }
  .state-edit-form {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.25rem 0;
  }
  .state-edit-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .state-key-input, .state-value-input {
    font-family: var(--iwe-font-ui);
    font-size: 0.82rem;
    padding: 0.15rem 0.4rem;
    height: 1.6rem;
    border: 1px solid var(--iwe-border, #ddd);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    color: var(--iwe-text);
    outline: none;
    box-sizing: border-box;
  }
  .state-key-input { width: 100%; }
  .state-value-input { flex: 1; min-width: 0; }
  .state-key-input:focus, .state-value-input:focus { border-color: var(--iwe-accent); }
  .state-note-input {
    width: 100%;
    font-family: var(--iwe-font-ui);
    font-size: 0.8rem;
    padding: 0.25rem 0.4rem;
    border: 1px solid var(--iwe-border, #ddd);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    color: var(--iwe-text);
    outline: none;
    resize: vertical;
  }
  .state-note-input:focus { border-color: var(--iwe-accent); }
  .state-edit-actions {
    display: flex;
    gap: 0.3rem;
  }
  .state-done-btn {
    font-family: var(--iwe-font-ui);
    font-size: 0.8rem;
    padding: 0.3rem 0.8rem;
    border: 1px solid var(--iwe-accent, #2d6a5e);
    border-radius: 4px;
    background: var(--iwe-accent, #2d6a5e);
    color: #fff;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-weight: 600;
  }
  .state-done-btn:hover { opacity: 0.9; }
  .state-footer-left {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .state-cancel-btn {
    font-family: var(--iwe-font-ui);
    font-size: 0.8rem;
    padding: 0.3rem 0.8rem;
    border: 1px solid var(--iwe-border, #ddd);
    border-radius: 4px;
    background: none;
    color: var(--iwe-text-secondary);
    cursor: pointer;
  }
  .state-cancel-btn:hover { background: var(--iwe-bg-hover); }
  .state-sub-tabs {
    display: flex;
    gap: 0;
    margin-bottom: 0.6rem;
    border-bottom: 1px solid var(--iwe-border-light, #eee);
  }
  .state-sub-tab {
    flex: 1;
    font-family: var(--iwe-font-ui);
    font-size: 0.75rem;
    padding: 0.35rem 0.4rem;
    border: none;
    background: none;
    color: var(--iwe-text-faint);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s;
  }
  .state-sub-tab:hover { color: var(--iwe-text-secondary); }
  .state-sub-tab.active {
    color: var(--iwe-accent, #2d6a5e);
    border-bottom-color: var(--iwe-accent, #2d6a5e);
    font-weight: 600;
  }
  .state-view-container {
    padding: 0.5rem 0.75rem;
  }
  .state-view-header {
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--iwe-text-faint);
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
  }
  .state-filter-row {
    display: flex;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
  }
  .state-filter-input {
    flex: 1;
    font-family: var(--iwe-font-ui);
    font-size: 0.78rem;
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--iwe-border, #ddd);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    color: var(--iwe-text);
    outline: none;
  }
  .state-filter-input:focus { border-color: var(--iwe-accent); }
  .state-filter-input::placeholder { color: var(--iwe-text-faint); }
  .state-resolved-list {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .state-resolved-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0.4rem 0.6rem;
    background: var(--iwe-bg, #fffef9);
    border: 1px solid var(--iwe-border-light, #eee);
    border-radius: 6px;
  }
  .state-resolved-key {
    font-family: var(--iwe-font-ui);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--iwe-text);
  }
  .state-resolved-value {
    font-family: var(--iwe-font-prose);
    font-size: 0.82rem;
    color: var(--iwe-accent, #2d6a5e);
  }
  .state-marker-edit {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.5rem 0.75rem;
  }
  .state-marker-edit-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 0.6rem;
    margin-top: 0.4rem;
    border-top: 1px solid var(--iwe-border-light, #eee);
  }
  .state-marker-edit-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--iwe-text-secondary);
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding-bottom: 0.2rem;
    border-bottom: 1px solid var(--iwe-border-subtle, #eee);
  }
  .state-edit-item {
    display: flex;
    align-items: center;
    height: 2.2rem;
    padding: 0 0.5rem;
    background: var(--iwe-bg, #fffef9);
    border: 1px solid var(--iwe-border-light, #eee);
    border-radius: 6px;
    margin-bottom: 0.3rem;
  }
  .state-edit-left {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
  }
  .state-edit-right {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.3rem;
    justify-content: flex-end;
  }
  .state-edit-label {
    font-family: var(--iwe-font-ui);
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--iwe-text);
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .state-val-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.15rem 0;
  }
  .state-add-val-btn {
    background: none;
    border: 1px dashed var(--iwe-border, #ddd);
    border-radius: var(--iwe-radius-sm);
    cursor: pointer;
    font-size: 0.78rem;
    color: var(--iwe-text-faint);
    padding: 0.3rem 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    margin-top: 0.2rem;
  }
  .state-add-buttons {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin-top: 0.2rem;
  }
  .state-add-val-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .state-ref-icon {
    color: var(--iwe-text-faint);
    font-size: 0.75rem;
    flex-shrink: 0;
  }
  .state-ref-toggle {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    cursor: pointer;
    font-size: 0.82rem;
  }
  .state-ref-label {
    color: var(--iwe-text-secondary);
    font-family: var(--iwe-font-ui);
  }
  .state-ref-add-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0;
  }
  .state-entity-select {
    flex: 1;
    font-family: var(--iwe-font-ui);
    font-size: 0.82rem;
    padding: 0.25rem 0.4rem;
    border: 1px solid var(--iwe-border, #ddd);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    color: var(--iwe-text);
    outline: none;
  }
  .state-entity-select:focus { border-color: var(--iwe-accent); }
  .state-resolved-ref .state-resolved-key {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .state-ref-inactive {
    color: var(--iwe-text-faint) !important;
  }
  .state-change-card {
    background: var(--iwe-bg, #fffef9);
    border: 1px solid var(--iwe-border-light, #eee);
    border-radius: 6px;
    margin-bottom: 0.4rem;
    overflow: hidden;
    cursor: pointer;
    transition: border-color 0.15s;
  }
  .state-change-card:hover {
    border-color: var(--iwe-accent, #2d6a5e);
  }
  .state-change-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.3rem 0.5rem;
    background: var(--iwe-bg-active, #f5f2ed);
  }
  .state-change-chapter {
    font-family: var(--iwe-font-ui);
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--iwe-text);
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .state-change-edit {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--iwe-text-faint);
    font-size: 0.75rem;
    padding: 0.1rem 0.3rem;
    border-radius: var(--iwe-radius-sm);
  }
  .state-change-edit:hover { background: var(--iwe-hover); color: var(--iwe-text); }
  .state-change-values {
    padding: 0.3rem 0.5rem;
  }
  .state-change-val {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0.1rem 0;
    font-size: 0.8rem;
  }
  .state-change-key {
    font-family: var(--iwe-font-ui);
    font-weight: 600;
    color: var(--iwe-text);
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  .state-change-value {
    font-family: var(--iwe-font-prose);
    color: var(--iwe-accent, #2d6a5e);
  }
  .state-change-empty {
    padding: 0.3rem 0.5rem;
    font-size: 0.78rem;
    color: var(--iwe-text-faint);
    font-style: italic;
  }
  .state-incoming-section {
    margin-top: 1rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--iwe-border-light, #eee);
  }
  .state-incoming-header {
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--iwe-text-faint);
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-bottom: 0.4rem;
  }
  .state-incoming-row {
    display: flex;
    align-items: center;
    height: 2.2rem;
    padding: 0 0.5rem;
    background: var(--iwe-bg, #fffef9);
    border: 1px solid var(--iwe-border-light, #eee);
    border-radius: 6px;
    margin-bottom: 0.3rem;
  }
  .state-incoming-name {
    flex: 1;
    font-family: var(--iwe-font-ui);
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--iwe-text);
  }
  .state-incoming-status {
    font-family: var(--iwe-font-ui);
    font-size: 0.82rem;
    color: var(--iwe-accent, #2d6a5e);
  }
  .state-marker-list-label {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--iwe-text-faint);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.3rem 0;
  }
  .state-val-count {
    margin-left: auto;
    font-size: 0.7rem;
    color: var(--iwe-text-faint);
  }
  .state-marker-preview {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    padding: 0.2rem 0;
  }
  .state-preview-pill {
    font-size: 0.72rem;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    background: rgba(45, 106, 94, 0.06);
    color: var(--iwe-text-secondary);
    font-family: var(--iwe-font-ui);
  }
  .state-preview-more {
    font-size: 0.7rem;
    color: var(--iwe-text-faint);
    font-style: italic;
  }

</style>
