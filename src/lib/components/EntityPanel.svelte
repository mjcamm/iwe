<script>
  import { detectEntities, addIgnoredWord, findReferences, getEntityNotes, addEntityNote, deleteEntityNote, reorderEntityNotes, getEntityFreeNotes, addEntityFreeNote, updateEntityFreeNote, deleteEntityFreeNote, reorderEntityFreeNotes } from '$lib/db.js';
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
    onclearselection
  } = $props();

  let view = $state('list'); // 'list' | 'create' | 'edit' | 'detect' | 'references' | 'view'
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
  let viewTab = $state('excerpts'); // 'excerpts' | 'notes'
  let newNoteText = $state('');

  async function showEntityView(entity) {
    viewEntity = entity;
    viewNotesLoading = true;
    view = 'view';
    try {
      viewNotes = await getEntityNotes(entity.id);
      viewFreeNotes = await getEntityFreeNotes(entity.id);
    } catch {
      viewNotes = [];
      viewFreeNotes = [];
    }
    viewNotesLoading = false;
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
      candidates.sort((a, b) => b.count - a.count);
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
    <!-- List View -->
    <div class="panel-header">
      <span class="panel-label">Entities</span>
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

    {#if selectedText}
      <div class="selection-hint">
        Selected: <strong>{selectedText.length > 30 ? selectedText.slice(0, 30) + '...' : selectedText}</strong>
      </div>
    {/if}

    <div class="panel-filters">
      <input class="filter-input" bind:value={filterText} placeholder="Filter entities..." />
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
              <div class="entity-item" class:viewed={entity.visible}>
                <span class="entity-color-dot" style="background: {entity.color}"></span>
                <div class="entity-info">
                  <span class="entity-name">{entity.name}</span>
                  {#if entity.aliases.length > 0}
                    <span class="entity-aliases">
                      {entity.aliases.join(', ')}
                    </span>
                  {/if}
                </div>
                <div class="entity-actions">
                  <button
                    class="entity-action-btn"
                    class:active={entity.visible}
                    onclick={() => ontoggleview?.(entity.id)}
                    title={entity.visible ? 'Hide highlights' : 'Show highlights'}
                  >
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      {#if entity.visible}
                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                        <circle cx="12" cy="12" r="3"/>
                      {:else}
                        <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/>
                        <path d="M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19"/>
                        <line x1="1" y1="1" x2="23" y2="23"/>
                      {/if}
                    </svg>
                  </button>
                  {#if selectedText}
                    <button
                      class="entity-action-btn pin-btn"
                      onclick={async () => { await pinSelectedText(entity.id); }}
                      title={`Pin selection to ${entity.name}`}
                    >
                      <i class="bi bi-pin-angle" style="font-size: 0.85rem;"></i>
                    </button>
                  {/if}
                  <button
                    class="entity-action-btn"
                    onclick={() => showEntityView(entity)}
                    title={`View ${entity.name}`}
                  >
                    <i class="bi bi-folder2-open" style="font-size: 0.85rem;"></i>
                  </button>
                  <button
                    class="entity-action-btn"
                    onclick={() => showReferences(entity)}
                    title="Find all references"
                  >
                    <i class="bi bi-search" style="font-size: 0.85rem;"></i>
                  </button>
                  <button
                    class="entity-action-btn"
                    onclick={() => startEdit(entity)}
                    title="Edit entity"
                  >
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M17 3l4 4-12 12H5v-4L17 3z" stroke-linejoin="round"/>
                    </svg>
                  </button>
                </div>
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

  {:else if view === 'edit'}
    <!-- Edit View -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">Edit Entity</span>
    </div>

    <div class="entity-form">
      <label class="form-label">
        Name
        <input class="form-input" bind:value={editName} placeholder="Entity name..."
          onblur={submitEdit} />
      </label>

      <label class="form-label">
        Type
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
      </label>

      <label class="form-label">
        Color
        <div class="color-pick">
          <input type="color" class="color-input" bind:value={editColor} onchange={submitEdit} />
          <span class="color-hex">{editColor}</span>
          <button class="btn-text-sm" onclick={() => { editColor = defaultColors[editType]; submitEdit(); }}>Reset</button>
        </div>
      </label>

      <label class="form-label">
        Aliases
        <div class="alias-list">
          {#each editingEntity?.aliases || [] as alias}
            <div class="alias-tag">
              <span>{alias}</span>
              <button class="alias-remove" onclick={() => handleRemoveAlias(alias)}>&times;</button>
            </div>
          {/each}
        </div>
        <form class="alias-add" onsubmit={e => { e.preventDefault(); handleAddAlias(); }}>
          <input class="form-input" bind:value={newAlias} placeholder="Add alias..." />
          <button class="btn-author btn-author-subtle" type="submit">Add</button>
        </form>
      </label>

      <label class="form-label">
        Description
        <textarea class="form-textarea" bind:value={editDescription} placeholder="Notes..."
          rows="4" onblur={submitEdit}></textarea>
      </label>

      <div class="form-actions">
        {#if showDeleteConfirm}
          <div class="delete-confirm">
            <p class="delete-confirm-msg">Delete <strong>"{editingEntity.name}"</strong>?</p>
            <div class="delete-confirm-actions">
              <button class="btn-danger" onclick={confirmDeleteEntity}>Delete</button>
              <button class="btn-author btn-author-subtle btn-author-sm" onclick={cancelDeleteEntity}>Cancel</button>
            </div>
          </div>
        {:else}
          <button class="btn-danger" onclick={handleDelete}>Delete Entity</button>
        {/if}
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
      <span class="panel-label">References</span>
    </div>

    {#if refsEntity}
      <div class="refs-entity-header">
        <span class="refs-color" style="background: {refsEntity.color}"></span>
        <span class="refs-name">{refsEntity.name}</span>
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
    <!-- Entity View Pane -->
    <div class="panel-header">
      <button class="back-btn" onclick={() => view = 'list'}>&larr;</button>
      <span class="panel-label">Entity</span>
      <div class="view-header-actions">
        <button class="entity-action-btn" onclick={() => showReferences(viewEntity)} title="Find references">
          <i class="bi bi-search" style="font-size: 0.85rem;"></i>
        </button>
        <button class="entity-action-btn" onclick={() => startEdit(viewEntity)} title="Edit">
          <i class="bi bi-pencil" style="font-size: 0.85rem;"></i>
        </button>
      </div>
    </div>

    {#if viewEntity}
      <div class="view-entity-header">
        <span class="view-color" style="background: {viewEntity.color}"></span>
        <div class="view-entity-info">
          <span class="view-name">{viewEntity.name}</span>
          <span class="view-type">{viewEntity.entity_type}</span>
        </div>
      </div>

      {#if viewEntity.aliases.length > 0}
        <div class="view-aliases">
          {#each viewEntity.aliases as alias}
            <span class="view-alias-tag">{alias}</span>
          {/each}
        </div>
      {/if}

      {#if viewEntity.description}
        <div class="view-description">{viewEntity.description}</div>
      {/if}

      <!-- Pin selected text -->
      {#if selectedText}
        <div class="view-pin-bar">
          <span class="view-pin-preview">&ldquo;{selectedText.length > 60 ? selectedText.slice(0, 60) + '...' : selectedText}&rdquo;</span>
          <button class="view-pin-btn" onclick={() => pinSelectedText(viewEntity.id)}>
            <i class="bi bi-pin-angle"></i> Pin
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
      {/if}
    {/if}
  {/if}
</div>

<style>
  .entity-panel {
    display: flex; flex-direction: column; height: 100%;
    font-family: var(--iwe-font-ui);
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
  .panel-filters { padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light); }
  .filter-input {
    width: 100%; padding: 0.35rem 0.5rem; font-size: 0.8rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    font-family: var(--iwe-font-ui); outline: none; margin-bottom: 0.4rem;
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
  .entity-list { flex: 1; overflow-y: auto; padding: 0.25rem 0; }

  .entity-group { margin-bottom: 0.25rem; }
  .group-header {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.35rem 0.75rem;
  }
  .group-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .group-label {
    font-size: 0.65rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.06em; color: var(--iwe-text-faint);
  }
  .group-count { font-size: 0.6rem; color: var(--iwe-text-faint); }

  .entity-item {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%;
    padding: 0.35rem 0.5rem 0.35rem 1.5rem;
    transition: background 100ms;
    font-family: var(--iwe-font-ui);
  }
  .entity-item:hover { background: var(--iwe-bg-hover); }
  .entity-item:hover .entity-actions { opacity: 1; }
  .entity-item.viewed { background: var(--iwe-bg-hover); }
  .entity-item.viewed .entity-actions { opacity: 1; }

  .entity-color-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
  }
  .entity-info {
    display: flex; flex-direction: column; gap: 0.1rem; min-width: 0; flex: 1;
  }
  .entity-name { font-size: 0.85rem; color: var(--iwe-text); }
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
</style>
