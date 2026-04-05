<script>
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { dndzone } from 'svelte-dnd-action';
  import {
    getChapters, addChapter, renameChapter, deleteChapter, reorderChapters,
    getEntities, createEntity, updateEntity, deleteEntity, addAlias, removeAlias,
    getAllChapterPlanningNotes, addChapterPlanningNote, updateChapterPlanningNote,
    deleteChapterPlanningNote, reorderChapterPlanningNotes, moveChapterPlanningNote,
    getEntityFreeNotes, addEntityFreeNote, updateEntityFreeNote,
    deleteEntityFreeNote, moveEntityFreeNote, reorderEntityFreeNotes,
    getKanbanColumns, addKanbanColumn, updateKanbanColumn,
    deleteKanbanColumn, reorderKanbanColumns,
    getAllKanbanCards, addKanbanCard, updateKanbanCard,
    deleteKanbanCard, moveKanbanCard, reorderKanbanCards,
  } from '$lib/db.js';
  import KanbanCardModal from '$lib/components/KanbanCardModal.svelte';

  let board = $state('chapters'); // 'chapters' | 'entities' | 'freeform'
  let columns = $state([]);
  let loading = $state(true);
  let modalCard = $state(null);
  let modalColumnId = $state(null);
  let modalIsNew = $state(false);

  // Column add/rename (freeform + chapters)
  let justAddedColId = $state(null);
  let addingColumn = $state(false);
  let newColumnTitle = $state('');
  let renamingColumnId = $state(null);
  let renameColumnTitle = $state('');

  // Entity edit modal
  let editingEntity = $state(null);
  let editEntityName = $state('');
  let editEntityType = $state('character');
  let editEntityColor = $state('');
  let editEntityDescription = $state('');
  let editEntityAliases = $state([]);
  let newAliasText = $state('');

  const flipDurationMs = 200;
  const defaultColors = { character: '#2d6a5e', place: '#6a4c2d', thing: '#4c2d6a' };
  const nextFrame = () => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));

  // Hand-grab scrolling
  let boardEl = $state(null);
  let grabbing = $state(false);
  let grabStartX = 0;
  let grabScrollLeft = 0;

  function handleGrabStart(e) {
    // Only grab on the board background itself, not on columns/cards
    if (e.target !== boardEl) return;
    grabbing = true;
    grabStartX = e.clientX;
    grabScrollLeft = boardEl.scrollLeft;
    e.preventDefault();
  }

  function handleGrabMove(e) {
    if (!grabbing) return;
    const dx = e.clientX - grabStartX;
    boardEl.scrollLeft = grabScrollLeft - dx;
  }

  function handleGrabEnd() {
    grabbing = false;
  }

  onMount(() => { loadBoard(); });

  $effect(() => { const _b = board; loadBoard(); });

  async function loadBoard() {
    loading = true;
    try {
      if (board === 'chapters') {
        const chapters = await getChapters();
        const allNotes = await getAllChapterPlanningNotes();
        columns = chapters.map(ch => ({
          id: ch.id,
          title: ch.title,
          items: allNotes
            .filter(n => n.chapter_id === ch.id)
            .map(n => ({ ...n, id: n.id })),
        }));
      } else if (board === 'entities') {
        const entities = await getEntities();
        const cols = [];
        for (const ent of entities) {
          const notes = await getEntityFreeNotes(ent.id);
          cols.push({
            id: ent.id,
            title: ent.name,
            color: ent.color,
            entityType: ent.entity_type,
            aliases: ent.aliases || [],
            description: ent.description || '',
            items: notes.map(n => ({
              ...n,
              id: n.id,
              description: n.text,
            })),
          });
        }
        columns = cols;
      } else {
        const cols = await getKanbanColumns();
        const allCards = await getAllKanbanCards();
        columns = cols.map(c => ({
          id: c.id,
          title: c.title,
          items: allCards
            .filter(card => card.column_id === c.id)
            .map(card => ({ ...card, id: card.id })),
        }));
      }
    } catch (e) {
      console.error('[kanban] load failed:', e);
    }
    loading = false;
  }

  // Column DnD
  function handleColumnConsider(e) {
    columns = e.detail.items;
  }

  async function handleColumnFinalize(e) {
    columns = e.detail.items;
    const ids = columns.map(c => c.id);
    if (board === 'chapters') {
      await reorderChapters(ids);
    } else if (board === 'freeform') {
      await reorderKanbanColumns(ids);
    }
    // entities: no reorder persisted (sorted by name)
  }

  // Card DnD
  function handleCardConsider(colId, e) {
    const idx = columns.findIndex(c => c.id === colId);
    columns[idx].items = e.detail.items;
    columns = [...columns];
  }

  async function handleCardFinalize(colId, e) {
    const idx = columns.findIndex(c => c.id === colId);
    columns[idx].items = e.detail.items;
    columns = [...columns];

    const items = columns[idx].items;

    for (const item of items) {
      const originalColId = item.chapter_id ?? item.entity_id ?? item.column_id;
      if (originalColId !== undefined && originalColId !== colId) {
        if (board === 'chapters') {
          await moveChapterPlanningNote(item.id, colId);
          item.chapter_id = colId;
        } else if (board === 'entities') {
          await moveEntityFreeNote(item.id, colId);
          item.entity_id = colId;
        } else {
          await moveKanbanCard(item.id, colId, items.indexOf(item));
          item.column_id = colId;
        }
      }
    }

    const ids = items.map(i => i.id);
    if (board === 'chapters') {
      await reorderChapterPlanningNotes(ids);
    } else if (board === 'entities') {
      await reorderEntityFreeNotes(ids);
    } else {
      await reorderKanbanCards(ids);
    }
  }

  // Add card
  function startAddCard(colId) {
    modalColumnId = colId;
    modalCard = null;
    modalIsNew = true;
  }

  function openCard(card, colId) {
    modalCard = { ...card };
    modalColumnId = colId;
    modalIsNew = false;
  }

  async function handleCardCreate({ description }) {
    const colId = modalColumnId;
    if (board === 'chapters') {
      await addChapterPlanningNote(colId, '', description);
    } else if (board === 'entities') {
      await addEntityFreeNote(colId, '', description);
    } else {
      await addKanbanCard(colId, '', description);
    }
    modalIsNew = false;
    modalCard = null;
    await loadBoard();
  }

  async function handleCardSave(updated) {
    if (board === 'chapters') {
      await updateChapterPlanningNote(updated.id, '', updated.description);
    } else if (board === 'entities') {
      await updateEntityFreeNote(updated.id, '', updated.description);
    } else {
      await updateKanbanCard(updated.id, '', updated.description);
    }
    await loadBoard();
  }

  async function handleCardDelete(card) {
    if (board === 'chapters') {
      await deleteChapterPlanningNote(card.id);
    } else if (board === 'entities') {
      await deleteEntityFreeNote(card.id);
    } else {
      await deleteKanbanCard(card.id);
    }
    modalCard = null;
    await loadBoard();
  }

  // --- Chapter column management ---
  async function handleAddChapter() {
    const num = columns.length + 1;
    const title = `Chapter ${num}`;
    const newId = await addChapter(title);
    // Optimistically append — no full reload
    columns = [...columns, { id: newId, title, items: [] }];
    justAddedColId = newId;
    setTimeout(() => { justAddedColId = null; }, 500);
    await nextFrame();
    if (boardEl) boardEl.scrollTo({ left: boardEl.scrollWidth, behavior: 'smooth' });
  }

  function startRenameColumn(col) {
    renamingColumnId = col.id;
    renameColumnTitle = col.title;
    setTimeout(() => document.querySelector('.col-rename-input')?.focus(), 50);
  }

  async function confirmRenameColumn() {
    if (!renameColumnTitle.trim()) { renamingColumnId = null; return; }
    if (board === 'chapters') {
      await renameChapter(renamingColumnId, renameColumnTitle.trim());
    } else if (board === 'freeform') {
      await updateKanbanColumn(renamingColumnId, renameColumnTitle.trim());
    }
    renamingColumnId = null;
    await loadBoard();
  }

  // --- Freeform column management ---
  async function handleAddFreeformColumn() {
    if (!newColumnTitle.trim()) { addingColumn = false; return; }
    const title = newColumnTitle.trim();
    const newId = await addKanbanColumn(title);
    addingColumn = false;
    newColumnTitle = '';
    columns = [...columns, { id: newId, title, items: [] }];
    justAddedColId = newId;
    setTimeout(() => { justAddedColId = null; }, 500);
    await nextFrame();
    if (boardEl) boardEl.scrollTo({ left: boardEl.scrollWidth, behavior: 'smooth' });
  }

  let confirmDeleteColId = $state(null);

  async function handleDeleteColumn(colId) {
    if (confirmDeleteColId !== colId) { confirmDeleteColId = colId; return; }
    confirmDeleteColId = null;
    if (board === 'chapters') {
      await deleteChapter(colId);
    } else if (board === 'entities') {
      await deleteEntity(colId);
    } else {
      await deleteKanbanColumn(colId);
    }
    columns = columns.filter(c => c.id !== colId);
  }

  // --- Entity management ---
  async function handleAddEntity() {
    const num = columns.length + 1;
    const title = `Entity ${num}`;
    const color = defaultColors.character;
    const newId = await createEntity(title, 'character', '', color);
    // Optimistically append
    const newCol = { id: newId, title, color, entityType: 'character', aliases: [], description: '', items: [] };
    columns = [...columns, newCol];
    justAddedColId = newId;
    setTimeout(() => { justAddedColId = null; }, 500);
    await nextFrame();
    if (boardEl) boardEl.scrollTo({ left: boardEl.scrollWidth, behavior: 'smooth' });
    // Open edit modal
    startEditEntity(newCol);
  }

  function startEditEntity(col) {
    editingEntity = col;
    editEntityName = col.title;
    editEntityType = col.entityType || 'character';
    editEntityColor = col.color || defaultColors[col.entityType] || '#666';
    editEntityDescription = col.description || '';
    editEntityAliases = [...(col.aliases || [])];
    newAliasText = '';
  }

  async function saveEntity() {
    if (!editingEntity || !editEntityName.trim()) return;
    await updateEntity(editingEntity.id, editEntityName.trim(), editEntityType, editEntityDescription, editEntityColor);
    editingEntity = null;
    await loadBoard();
  }

  function setEntityType(type) {
    editEntityType = type;
    editEntityColor = defaultColors[type] || '#666';
  }

  async function handleAddAlias() {
    if (!editingEntity || !newAliasText.trim()) return;
    await addAlias(editingEntity.id, newAliasText.trim());
    editEntityAliases = [...editEntityAliases, newAliasText.trim()];
    newAliasText = '';
  }

  async function handleRemoveAlias(alias) {
    if (!editingEntity) return;
    await removeAlias(editingEntity.id, alias);
    editEntityAliases = editEntityAliases.filter(a => a !== alias);
  }

  function truncate(text, max = 80) {
    if (!text) return '';
    const firstLine = text.split('\n')[0];
    if (firstLine.length <= max) return firstLine;
    return firstLine.slice(0, max).trim() + '...';
  }
</script>

<div class="kanban-page">
  <div class="kanban-header">
    <div class="board-toggle">
      <button class="board-btn" class:active={board === 'chapters'} onclick={() => board = 'chapters'}>
        <i class="bi bi-journal-text"></i> Chapters
      </button>
      <button class="board-btn" class:active={board === 'entities'} onclick={() => board = 'entities'}>
        <i class="bi bi-people"></i> Entities
      </button>
      <button class="board-btn" class:active={board === 'freeform'} onclick={() => board = 'freeform'}>
        <i class="bi bi-kanban"></i> Freeform
      </button>
    </div>

  </div>

  {#if loading}
    <div class="kanban-loading">Loading board...</div>
  {:else if columns.length === 0}
    <div class="board" bind:this={boardEl}>
      {#if board === 'chapters'}
        <button class="add-column-placeholder" onclick={handleAddChapter}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Chapter</span>
        </button>
      {:else if board === 'entities'}
        <button class="add-column-placeholder" onclick={handleAddEntity}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Entity</span>
        </button>
      {:else}
        <button class="add-column-placeholder" onclick={() => { addingColumn = true; newColumnTitle = ''; }}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Column</span>
        </button>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="board"
      class:grabbing
      bind:this={boardEl}
      onmousedown={handleGrabStart}
      onmousemove={handleGrabMove}
      onmouseup={handleGrabEnd}
      onmouseleave={handleGrabEnd}
      use:dndzone={{ items: columns, flipDurationMs, type: 'columns', dropTargetStyle: {} }}
      onconsider={handleColumnConsider}
      onfinalize={handleColumnFinalize}>
      {#each columns as col (col.id)}
        <div class="column" class:just-added={col.id === justAddedColId} animate:flip={{ duration: flipDurationMs }}>
          <div class="column-header">
            {#if renamingColumnId === col.id}
              <input
                class="col-rename-input"
                type="text"
                bind:value={renameColumnTitle}
                onkeydown={e => { if (e.key === 'Enter') confirmRenameColumn(); if (e.key === 'Escape') renamingColumnId = null; }}
                onblur={confirmRenameColumn}
              />
            {:else}
              {#if board === 'entities' && col.color}
                <span class="entity-dot" style="background: {col.color};"></span>
              {/if}
              <span class="column-title">
                {col.title}
              </span>
              <span class="column-count">{col.items.length}</span>
              <div class="column-actions">
                {#if board === 'entities'}
                  <button class="col-action" onclick={() => startEditEntity(col)} title="Edit entity">
                    <i class="bi bi-pencil"></i>
                  </button>
                {:else}
                  <button class="col-action" onclick={() => startRenameColumn(col)} title="Rename">
                    <i class="bi bi-pencil"></i>
                  </button>
                {/if}
                {#if confirmDeleteColId === col.id}
                  <button class="col-action col-action-confirm-delete" onclick={() => handleDeleteColumn(col.id)} title="Confirm delete">
                    <i class="bi bi-check-lg"></i>
                  </button>
                  <button class="col-action" onclick={() => confirmDeleteColId = null} title="Cancel">
                    <i class="bi bi-x-lg"></i>
                  </button>
                {:else}
                  <button class="col-action" onclick={() => handleDeleteColumn(col.id)} title="Delete">
                    <i class="bi bi-trash3"></i>
                  </button>
                {/if}
              </div>
            {/if}
          </div>

          <div class="column-content"
            use:dndzone={{ items: col.items, flipDurationMs, group: 'cards', dropTargetStyle: { outline: '2px dashed #2d6a5e', 'outline-offset': '-2px', 'background': 'rgba(45, 106, 94, 0.06)' } }}
            onconsider={e => handleCardConsider(col.id, e)}
            onfinalize={e => handleCardFinalize(col.id, e)}>
            {#each col.items as item (item.id)}
              <div class="card" animate:flip={{ duration: flipDurationMs }} onclick={() => openCard(item, col.id)}>
                <div class="card-text">{truncate(item.description || item.text) || '(empty)'}</div>
              </div>
            {/each}
          </div>

          <button class="add-card-btn" onclick={() => startAddCard(col.id)}>
            <i class="bi bi-plus"></i> Add card
          </button>
        </div>
      {/each}

      <!-- Add column placeholder -->
      {#if board === 'chapters'}
        <button class="add-column-placeholder" onclick={handleAddChapter}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Chapter</span>
        </button>
      {:else if board === 'entities'}
        <button class="add-column-placeholder" onclick={handleAddEntity}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Entity</span>
        </button>
      {:else}
        <button class="add-column-placeholder" onclick={() => { addingColumn = true; newColumnTitle = ''; }}>
          <i class="bi bi-plus-lg"></i>
          <span>Add Column</span>
        </button>
      {/if}
    </div>
  {/if}
</div>

<KanbanCardModal
  show={!!modalCard || modalIsNew}
  card={modalCard}
  isNew={modalIsNew}
  onCreate={handleCardCreate}
  onSave={handleCardSave}
  onDelete={handleCardDelete}
  onClose={() => { modalCard = null; modalIsNew = false; }}
/>

<!-- Entity edit modal -->
{#if editingEntity}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ent-backdrop" onclick={e => { if (e.target === e.currentTarget) saveEntity(); }}>
    <div class="ent-modal">
      <div class="ent-header">
        <span class="ent-header-title">Edit Entity</span>
        <button class="ent-close" onclick={saveEntity}>&times;</button>
      </div>
      <div class="ent-body">
        <label class="ent-label">Name</label>
        <input class="ent-input" type="text" bind:value={editEntityName} />

        <label class="ent-label">Type</label>
        <div class="ent-type-row">
          <button class="ent-type-btn" class:active={editEntityType === 'character'} onclick={() => setEntityType('character')}>Character</button>
          <button class="ent-type-btn" class:active={editEntityType === 'place'} onclick={() => setEntityType('place')}>Place</button>
          <button class="ent-type-btn" class:active={editEntityType === 'thing'} onclick={() => setEntityType('thing')}>Thing</button>
        </div>

        <label class="ent-label">Color</label>
        <div class="ent-color-row">
          <input class="ent-color-picker" type="color" bind:value={editEntityColor} />
          <span class="ent-color-hex">{editEntityColor}</span>
        </div>

        <label class="ent-label">Aliases</label>
        <div class="ent-aliases">
          {#each editEntityAliases as alias}
            <span class="ent-alias-tag">
              {alias}
              <button class="ent-alias-remove" onclick={() => handleRemoveAlias(alias)}>&times;</button>
            </span>
          {/each}
        </div>
        <div class="ent-alias-add">
          <input class="ent-input ent-alias-input" type="text" placeholder="Add alias..." bind:value={newAliasText}
            onkeydown={e => { if (e.key === 'Enter') { e.preventDefault(); handleAddAlias(); } }} />
          <button class="ent-alias-btn" onclick={handleAddAlias} disabled={!newAliasText.trim()}>
            <i class="bi bi-plus"></i>
          </button>
        </div>

        <label class="ent-label">Description</label>
        <textarea class="ent-textarea" bind:value={editEntityDescription} rows="3" placeholder="Optional description..."></textarea>
      </div>
      <div class="ent-footer">
        <button class="ent-done" onclick={saveEntity}>Done</button>
      </div>
    </div>
  </div>
{/if}

<!-- Add column modal (freeform) -->
{#if addingColumn}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ent-backdrop" onclick={e => { if (e.target === e.currentTarget) { addingColumn = false; } }}>
    <div class="ent-modal">
      <div class="ent-header">
        <span class="ent-header-title">New Column</span>
        <button class="ent-close" onclick={() => addingColumn = false}>&times;</button>
      </div>
      <div class="ent-body">
        <label class="ent-label">Title</label>
        <input class="ent-input" type="text" placeholder="Column title..." bind:value={newColumnTitle}
          onkeydown={e => { if (e.key === 'Enter') handleAddFreeformColumn(); if (e.key === 'Escape') addingColumn = false; }} />
      </div>
      <div class="ent-footer">
        <button class="km-cancel" onclick={() => addingColumn = false}>Cancel</button>
        <button class="ent-done" onclick={handleAddFreeformColumn} disabled={!newColumnTitle.trim()}>Create</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .kanban-page {
    display: flex; flex-direction: column;
    height: 100%; background: #faf8f5;
    overflow: hidden;
  }

  .kanban-header {
    display: flex; align-items: center; justify-content: center; gap: 1rem;
    padding: 0.8rem 1rem;
    border-bottom: 1px solid #e5e1da;
    background: white;
    flex-shrink: 0;
  }

  .board-toggle {
    display: flex; gap: 2px;
    background: #f0ede8; border-radius: 6px; padding: 2px;
  }
  .board-btn {
    font-family: 'Source Sans 3', system-ui; font-size: 0.82rem;
    padding: 0.35rem 0.8rem; border: none; border-radius: 5px;
    cursor: pointer; background: none; color: #6b6560;
    display: flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .board-btn:hover { color: #2d2a26; }
  .board-btn.active {
    background: white; color: #2d6a5e; font-weight: 500;
    box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }

  .kanban-loading, .kanban-empty {
    display: flex; align-items: center; justify-content: center;
    flex: 1; color: #9e9891; font-style: italic;
    font-family: 'Source Sans 3', system-ui; font-size: 0.9rem;
  }

  .board {
    display: flex; gap: 0.75rem;
    padding: 1.5rem 1rem 1rem;
    flex: 1; overflow-x: auto; overflow-y: hidden;
    align-items: flex-start;
    cursor: grab;
  }
  .board.grabbing { cursor: grabbing; }
  .board.grabbing * { user-select: none; }

  .column {
    flex-shrink: 0; width: 380px;
    background: #f0ede8; border-radius: 8px;
    display: flex; flex-direction: column;
    max-height: 100%;
  }
  .column.just-added {
    animation: col-enter 0.35s cubic-bezier(0.22, 1, 0.36, 1);
  }
  @keyframes col-enter {
    0% { opacity: 0; transform: scale(0.92); max-width: 0; margin: 0; padding: 0; overflow: hidden; }
    40% { max-width: 380px; margin: initial; padding: initial; overflow: visible; opacity: 0.3; transform: scale(0.96); }
    100% { opacity: 1; transform: scale(1); }
  }

  .column-header {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.6rem 0.7rem;
    flex-shrink: 0;
  }
  .entity-dot {
    width: 10px; height: 10px; border-radius: 50%;
    flex-shrink: 0;
  }
  .column-title {
    font-family: 'Libre Baskerville', Georgia, serif; font-size: 1rem;
    font-weight: 600; color: #2d2a26;
    flex: 1; min-width: 0; overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap;
  }
  .column-count {
    font-family: 'Source Sans 3', system-ui; font-size: 0.78rem;
    color: #9e9891; background: #e5e1da; border-radius: 10px;
    padding: 0.15rem 0.5rem; flex-shrink: 0;
  }
  .column-actions { display: flex; gap: 0.1rem; flex-shrink: 0; }
  .col-action {
    background: none; border: none; cursor: pointer;
    color: #c8c3bb; font-size: 1.05rem; padding: 0.25rem 0.4rem;
    border-radius: 4px;
  }
  .col-action:hover { color: #6b6560; background: #e5e1da; }
  .col-action-confirm-delete { color: #b85450; }
  .col-action-confirm-delete:hover { color: white; background: #b85450; }
  .col-rename-input {
    flex: 1; font-family: 'Source Sans 3', system-ui; font-size: 0.82rem;
    font-weight: 600; padding: 0.2rem 0.4rem;
    border: 1px solid #2d6a5e; border-radius: 4px; outline: none;
  }

  .column-content {
    flex: 1; overflow-y: auto;
    padding: 0 0.4rem;
    min-height: 40px;
  }

  .card {
    background: white; border: 1px solid #e5e1da;
    border-radius: 6px; padding: 0.5rem 0.6rem;
    margin-bottom: 0.4rem; cursor: pointer;
    transition: box-shadow 0.15s, border-color 0.15s;
  }
  .card:hover {
    border-color: #c8c3bb;
    box-shadow: 0 2px 6px rgba(0,0,0,0.06);
  }
  .card-text {
    font-family: 'Libre Baskerville', Georgia, serif; font-size: 0.88rem;
    color: #3d3a37; line-height: 1.5;
    display: -webkit-box; -webkit-line-clamp: 4;
    -webkit-box-orient: vertical; overflow: hidden;
  }

  .add-card-btn {
    width: 100%; font-family: 'Source Sans 3', system-ui; font-size: 0.9rem;
    padding: 0.55rem; border: none; border-radius: 0 0 8px 8px;
    cursor: pointer; background: none; color: #9e9891;
    display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    transition: all 100ms;
  }
  .add-card-btn:hover { color: #2d6a5e; background: rgba(45, 106, 94, 0.06); }

  .add-column-placeholder {
    flex-shrink: 0; width: 380px; min-height: 120px;
    background: none; border: 2px dashed #d5d0c9;
    border-radius: 8px; cursor: pointer;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 0.4rem; color: #b5b0a8;
    font-family: 'Source Sans 3', system-ui; font-size: 0.85rem;
    transition: all 150ms;
  }
  .add-column-placeholder:hover {
    border-color: #2d6a5e; color: #2d6a5e;
    background: rgba(45, 106, 94, 0.03);
  }
  .add-column-placeholder i { font-size: 1.2rem; }

  /* Entity edit modal */
  .ent-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.35);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 8vh;
  }
  .ent-modal {
    background: white; border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2);
    width: 90vw; max-width: 440px;
    display: flex; flex-direction: column;
    animation: ent-slide 0.2s ease;
  }
  @keyframes ent-slide {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .ent-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.8rem 1.2rem;
    border-bottom: 1px solid #f0ede8;
  }
  .ent-header-title {
    font-family: 'Source Sans 3', system-ui; font-size: 0.85rem;
    font-weight: 600; color: #2d2a26;
  }
  .ent-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.4rem; line-height: 1; color: #c8c3bb; padding: 0.2rem;
  }
  .ent-close:hover { color: #6b6560; }
  .ent-body { padding: 1rem 1.2rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .ent-label {
    font-family: 'Source Sans 3', system-ui; font-size: 0.7rem;
    font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
    color: #9e9891; margin-top: 0.2rem;
  }
  .ent-input {
    font-family: 'Source Sans 3', system-ui; font-size: 0.9rem;
    padding: 0.4rem 0.6rem; border: 1px solid #e5e1da;
    border-radius: 5px; outline: none; color: #3d3a37;
  }
  .ent-input:focus { border-color: #2d6a5e; }
  .ent-type-row { display: flex; gap: 4px; }
  .ent-type-btn {
    font-family: 'Source Sans 3', system-ui; font-size: 0.78rem;
    padding: 0.3rem 0.7rem; border: 1px solid #e5e1da;
    border-radius: 5px; cursor: pointer; background: none; color: #6b6560;
  }
  .ent-type-btn.active { border-color: #2d6a5e; color: #2d6a5e; background: rgba(45, 106, 94, 0.06); font-weight: 500; }
  .ent-color-row { display: flex; align-items: center; gap: 0.5rem; }
  .ent-color-picker { width: 32px; height: 32px; border: 1px solid #e5e1da; border-radius: 4px; padding: 0; cursor: pointer; }
  .ent-color-hex { font-family: 'Source Sans 3', system-ui; font-size: 0.78rem; color: #9e9891; }
  .ent-aliases { display: flex; flex-wrap: wrap; gap: 0.3rem; }
  .ent-alias-tag {
    font-family: 'Source Sans 3', system-ui; font-size: 0.78rem;
    padding: 0.15rem 0.5rem; background: #f0ede8; border-radius: 4px;
    color: #3d3a37; display: flex; align-items: center; gap: 0.3rem;
  }
  .ent-alias-remove {
    background: none; border: none; cursor: pointer; color: #c8c3bb;
    font-size: 0.9rem; line-height: 1; padding: 0;
  }
  .ent-alias-remove:hover { color: #b85450; }
  .ent-alias-add { display: flex; gap: 0.3rem; }
  .ent-alias-input { flex: 1; }
  .ent-alias-btn {
    background: none; border: 1px solid #e5e1da; border-radius: 5px;
    cursor: pointer; padding: 0.3rem 0.5rem; color: #6b6560;
  }
  .ent-alias-btn:hover:not(:disabled) { border-color: #2d6a5e; color: #2d6a5e; }
  .ent-alias-btn:disabled { opacity: 0.3; cursor: default; }
  .ent-textarea {
    font-family: 'Source Sans 3', system-ui; font-size: 0.88rem;
    padding: 0.4rem 0.6rem; border: 1px solid #e5e1da;
    border-radius: 5px; outline: none; resize: vertical;
    color: #3d3a37; line-height: 1.5;
  }
  .ent-textarea:focus { border-color: #2d6a5e; }
  .ent-footer {
    display: flex; justify-content: flex-end;
    padding: 0.7rem 1.2rem; border-top: 1px solid #f0ede8;
  }
  .ent-done {
    font-family: 'Source Sans 3', system-ui; font-size: 0.78rem;
    padding: 0.35rem 1rem; border: 1px solid #2d6a5e;
    border-radius: 5px; cursor: pointer;
    background: #2d6a5e; color: white;
  }
  .ent-done:hover { opacity: 0.9; }
</style>
