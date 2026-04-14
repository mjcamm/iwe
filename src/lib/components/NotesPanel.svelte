<script>
  let { comments = [], planningNotes = [], activeNoteId = null, hasEditorSelection = false, ondelete, onupdate, onselectnote, onupdatehighlight, onupdateplanning, ondeleteplanning } = $props();

  let editText = $state('');
  let textareaEl = $state(null);
  let confirmingDelete = $state(false);
  let expandedPlanningId = $state(null);

  function truncateFirstLine(text, max = 80) {
    if (!text) return '';
    const firstLine = text.split('\n')[0];
    if (firstLine.length <= max) return firstLine;
    return firstLine.slice(0, max).trim() + '...';
  }

  function togglePlanning(id) {
    expandedPlanningId = expandedPlanningId === id ? null : id;
  }

  // When activeNoteId changes, load that note's text and reset delete confirm
  $effect(() => {
    const note = activeNoteId != null ? comments.find(c => c.id === activeNoteId) : null;
    editText = note?.note_text ?? '';
    confirmingDelete = false;
  });

  let activeNote = $derived(activeNoteId != null ? comments.find(c => c.id === activeNoteId) : null);

  function saveNote() {
    if (activeNoteId != null) {
      onupdate?.(activeNoteId, editText);
    }
  }

  function handleDelete() {
    if (confirmingDelete && activeNoteId != null) {
      ondelete?.(activeNoteId);
      confirmingDelete = false;
    } else {
      confirmingDelete = true;
    }
  }

  function cancelDelete() {
    confirmingDelete = false;
  }

  function formatDate(dateStr) {
    if (!dateStr) return '';
    try {
      const d = new Date(dateStr);
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) + ' ' +
             d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    } catch { return dateStr; }
  }
</script>

<div class="notes-panel">
  {#if activeNote}
    <!-- Detail view: single note -->
    <div class="note-detail">
      <div class="note-detail-header">
        <button class="note-back" onclick={() => onselectnote?.(null)} title="Back to all notes">
          <i class="bi bi-arrow-left"></i>
        </button>
        <span class="note-detail-title">Note</span>
        <span class="note-detail-date">{formatDate(activeNote.created_at)}</span>
      </div>

      <div class="note-detail-body">
        <textarea
          bind:this={textareaEl}
          class="note-detail-textarea"
          bind:value={editText}
          onblur={saveNote}
          placeholder="Write your note here..."
        ></textarea>
        <div class="note-detail-actions">
          {#if confirmingDelete}
            <button class="note-action-btn note-action-confirm-delete" onclick={handleDelete}>
              <i class="bi bi-trash"></i>
              Confirm delete
            </button>
            <button class="note-action-btn" onclick={cancelDelete}>
              Cancel
            </button>
          {:else}
            <button class="note-action-btn note-action-delete" onclick={handleDelete} title="Delete note">
              <i class="bi bi-trash"></i>
              Delete
            </button>
          {/if}
          {#if hasEditorSelection}
            <button class="note-action-btn" onclick={() => onupdatehighlight?.(activeNoteId)} title="Update which text this note highlights to the current selection">
              <i class="bi bi-cursor-text"></i>
              Update highlight
            </button>
          {/if}
        </div>
      </div>
    </div>

  {:else}
    <!-- List view: all notes -->
    <div class="notes-header">
      <span class="notes-label">Inline Notes</span>
      <span class="notes-count">{comments.length}</span>
    </div>

    {#if comments.length === 0 && planningNotes.length === 0}
      <div class="notes-empty">
        <i class="bi bi-chat-left-text" style="font-size: 1.5rem; opacity: 0.3;"></i>
        <p>No notes yet</p>
        <p class="notes-empty-hint">Right-click in the editor to add a note</p>
      </div>
    {:else}
      {#if comments.length > 0}
        <div class="notes-list">
          {#each comments as comment (comment.id)}
            <button class="note-item" onclick={() => onselectnote?.(comment.id)}>
              <div class="note-item-color"></div>
              <div class="note-item-content">
                {#if comment.note_text}
                  <span class="note-item-text">{comment.note_text.length > 80 ? comment.note_text.slice(0, 80) + '...' : comment.note_text}</span>
                {:else}
                  <span class="note-item-empty">Empty note</span>
                {/if}
                <span class="note-item-date">{formatDate(comment.created_at)}</span>
              </div>
            </button>
          {/each}
        </div>
      {/if}

      {#if planningNotes.length > 0}
        <div class="planning-section">
          <div class="notes-header" style="padding-top: 0.5rem;">
            <span class="notes-label">Kanban Notes</span>
            <span class="notes-count">{planningNotes.length}</span>
          </div>
          <div class="notes-list">
            {#each planningNotes as pn (pn.id)}
              {@const expanded = expandedPlanningId === pn.id}
              <button class="planning-note-item" class:expanded onclick={() => togglePlanning(pn.id)}>
                <div class="note-item-color" style="background: rgba(45, 106, 94, 0.5);"></div>
                <div class="note-item-content">
                  {#if expanded && pn.description}
                    <span class="note-item-full">{pn.description}</span>
                  {:else}
                    <span class="note-item-text">{truncateFirstLine(pn.description) || '(empty)'}</span>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .notes-panel {
    padding: 0.5rem 0;
    height: 100%;
    overflow-y: auto;
  }

  /* ---- List view ---- */
  .notes-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.75rem 0.5rem;
  }
  .notes-label {
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--iwe-text-muted);
  }
  .notes-count {
    font-size: 0.65rem;
    background: var(--iwe-bg-active, #eee);
    color: var(--iwe-text-secondary);
    padding: 1px 6px;
    border-radius: 8px;
  }

  .notes-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem 1rem;
    color: var(--iwe-text-faint);
    text-align: center;
    gap: 0.3rem;
  }
  .notes-empty p { margin: 0; font-size: 0.9rem; }
  .notes-empty-hint { font-size: 0.75rem; opacity: 0.7; }

  .notes-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 0.4rem;
  }

  .note-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.45rem 0.5rem;
    background: none;
    border: none;
    border-radius: var(--iwe-radius-sm, 4px);
    cursor: pointer;
    text-align: left;
    font-family: var(--iwe-font-ui);
    color: var(--iwe-text-secondary);
    transition: background 0.12s;
  }
  .note-item:hover {
    background: var(--iwe-bg-hover, #f5f3f0);
  }

  .note-item-color {
    width: 3px;
    align-self: stretch;
    border-radius: 2px;
    background: rgba(255, 185, 0, 0.5);
    flex-shrink: 0;
  }
  .note-item-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .note-item-text {
    font-family: var(--iwe-font-prose);
    font-size: 0.95rem;
    line-height: 1.5;
    color: var(--iwe-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note-item-empty {
    font-family: var(--iwe-font-prose);
    font-size: 0.95rem;
    color: var(--iwe-text-faint);
    font-style: italic;
  }
  .note-item-date {
    font-size: 0.65rem;
    color: var(--iwe-text-faint);
  }
  /* ---- Detail view ---- */
  .note-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    animation: noteDetailIn 0.2s ease;
  }
  @keyframes noteDetailIn {
    from { opacity: 0; transform: translateX(8px); }
    to { opacity: 1; transform: translateX(0); }
  }

  .note-detail-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid var(--iwe-border, #e5e1da);
  }
  .note-back {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--iwe-text-secondary);
    font-size: 0.9rem;
    padding: 2px 6px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    transition: all 0.12s;
  }
  .note-back:hover {
    background: var(--iwe-bg-hover);
    color: var(--iwe-accent);
  }
  .note-detail-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--iwe-text);
    flex: 1;
  }
  .note-detail-date {
    font-size: 0.65rem;
    color: var(--iwe-text-faint);
  }

  .note-detail-body {
    flex: 1;
    padding: 0.5rem;
  }
  .note-detail-textarea {
    width: 100%;
    min-height: 120px;
    font-family: var(--iwe-font-prose);
    font-size: 1.05rem;
    line-height: 1.6;
    border: 1px solid var(--iwe-border, #e5e1da);
    border-radius: var(--iwe-radius-sm, 4px);
    padding: 0.55rem 0.6rem;
    resize: vertical;
    outline: none;
    background: var(--iwe-bg);
    color: var(--iwe-text);
    transition: border-color 0.15s;
  }
  .note-detail-textarea:focus {
    border-color: var(--iwe-accent, #2d6a5e);
  }

  .note-detail-actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding-top: 0.4rem;
  }
  .note-action-btn {
    background: var(--iwe-bg-hover, #f5f3f0);
    border: 1px solid var(--iwe-border, #e5e1da);
    font-size: 0.85rem;
    color: var(--iwe-text-secondary);
    cursor: pointer;
    padding: 6px 12px;
    border-radius: var(--iwe-radius-sm, 4px);
    display: flex;
    align-items: center;
    gap: 0.35rem;
    transition: all 0.15s;
    font-family: var(--iwe-font-ui);
  }
  .note-action-btn:hover {
    background: var(--iwe-bg-active, #ebe8e3);
    color: var(--iwe-text);
  }
  .note-action-delete:hover {
    border-color: var(--iwe-danger, #b85450);
    color: var(--iwe-danger, #b85450);
  }
  .note-action-confirm-delete {
    background: var(--iwe-danger, #b85450);
    border-color: var(--iwe-danger, #b85450);
    color: #fff;
  }
  .note-action-confirm-delete:hover {
    background: #a04040;
    border-color: #a04040;
    color: #fff;
  }

  /* Planning notes */
  .planning-section {
    border-top: 1px solid var(--iwe-border-light, #f0ede8);
    margin-top: 0.3rem;
  }
  .planning-note-item {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    width: 100%;
    padding: 0.45rem 0.5rem;
    background: none;
    border: none;
    border-radius: var(--iwe-radius-sm, 4px);
    font-family: var(--iwe-font-ui);
    color: var(--iwe-text-secondary);
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
  }
  .planning-note-item:hover {
    background: var(--iwe-bg-hover, #f5f3f0);
  }
  .planning-note-item.expanded {
    background: var(--iwe-bg-hover, #f5f3f0);
  }
  .note-item-full {
    font-family: var(--iwe-font-prose);
    font-size: 0.95rem;
    line-height: 1.5;
    color: var(--iwe-text);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .planning-note-desc {
    font-size: 0.72rem;
    color: var(--iwe-text-faint);
    line-height: 1.3;
  }
</style>
