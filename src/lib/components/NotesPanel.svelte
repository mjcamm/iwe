<script>
  let { comments = [], activeNoteId = null, ondelete, onupdate, onselectnote, onupdatehighlight } = $props();

  let editText = $state('');
  let textareaEl = $state(null);

  // When activeNoteId changes, load that note's text
  $effect(() => {
    const note = activeNoteId != null ? comments.find(c => c.id === activeNoteId) : null;
    editText = note?.note_text ?? '';
  });

  let activeNote = $derived(activeNoteId != null ? comments.find(c => c.id === activeNoteId) : null);

  function saveNote() {
    if (activeNoteId != null) {
      onupdate?.(activeNoteId, editText);
    }
  }

  function handleDelete() {
    if (activeNoteId != null && confirm('Delete this note?')) {
      ondelete?.(activeNoteId);
    }
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

      {#if activeNote.highlightLen > 0}
        <div class="note-detail-status">
          <i class="bi bi-link-45deg"></i> Anchored to text
        </div>
      {:else}
        <div class="note-detail-status">
          <i class="bi bi-pin-angle"></i> Pinned at position
        </div>
      {/if}

      <div class="note-detail-body">
        <textarea
          bind:this={textareaEl}
          class="note-detail-textarea"
          bind:value={editText}
          onblur={saveNote}
          placeholder="Write your note here..."
        ></textarea>
      </div>

      <div class="note-detail-actions">
        <button class="note-action" onclick={() => onupdatehighlight?.(activeNoteId)} title="Select text in editor, then click to anchor note to selection">
          <i class="bi bi-cursor-text"></i>
          Update highlight
        </button>
        <button class="note-action note-action-delete" onclick={handleDelete} title="Delete note">
          <i class="bi bi-trash"></i>
          Delete
        </button>
      </div>
    </div>

  {:else}
    <!-- List view: all notes -->
    <div class="notes-header">
      <span class="notes-label">Notes</span>
      <span class="notes-count">{comments.length}</span>
    </div>

    {#if comments.length === 0}
      <div class="notes-empty">
        <i class="bi bi-chat-left-text" style="font-size: 1.5rem; opacity: 0.3;"></i>
        <p>No notes yet</p>
        <p class="notes-empty-hint">Right-click in the editor to add a note</p>
      </div>
    {:else}
      <div class="notes-list">
        {#each comments as comment (comment.id)}
          <button class="note-item" onclick={() => onselectnote?.(comment.id)}>
            <i class="bi bi-chat-left-text-fill note-item-icon"></i>
            <div class="note-item-content">
              {#if comment.note_text}
                <span class="note-item-text">{comment.note_text.length > 80 ? comment.note_text.slice(0, 80) + '...' : comment.note_text}</span>
              {:else}
                <span class="note-item-empty">Empty note</span>
              {/if}
              <span class="note-item-date">{formatDate(comment.created_at)}</span>
            </div>
            {#if comment.highlightLen > 0}
              <i class="bi bi-link-45deg note-item-badge" title="Anchored"></i>
            {:else}
              <i class="bi bi-pin-angle note-item-badge" title="Pinned"></i>
            {/if}
          </button>
        {/each}
      </div>
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
  .notes-empty p { margin: 0; font-size: 0.85rem; }
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

  .note-item-icon {
    font-size: 0.75rem;
    color: var(--iwe-accent, #2d6a5e);
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
    font-size: 0.8rem;
    line-height: 1.4;
    color: var(--iwe-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note-item-empty {
    font-size: 0.8rem;
    color: var(--iwe-text-faint);
    font-style: italic;
  }
  .note-item-date {
    font-size: 0.65rem;
    color: var(--iwe-text-faint);
  }
  .note-item-badge {
    font-size: 0.65rem;
    color: var(--iwe-text-faint);
    flex-shrink: 0;
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
    font-size: 0.85rem;
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
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--iwe-text);
    flex: 1;
  }
  .note-detail-date {
    font-size: 0.65rem;
    color: var(--iwe-text-faint);
  }

  .note-detail-status {
    padding: 0.3rem 0.6rem;
    font-size: 0.7rem;
    color: var(--iwe-text-faint);
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .note-detail-body {
    flex: 1;
    padding: 0.5rem;
  }
  .note-detail-textarea {
    width: 100%;
    height: 100%;
    min-height: 120px;
    font-size: 0.85rem;
    line-height: 1.6;
    font-family: var(--iwe-font-ui);
    border: 1px solid var(--iwe-border, #e5e1da);
    border-radius: var(--iwe-radius-sm, 4px);
    padding: 0.5rem;
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
    gap: 0.3rem;
    padding: 0.4rem 0.5rem;
    border-top: 1px solid var(--iwe-border, #e5e1da);
  }
  .note-action {
    background: none;
    border: none;
    font-size: 0.72rem;
    color: var(--iwe-text-faint);
    cursor: pointer;
    padding: 3px 8px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    transition: all 0.15s;
    font-family: var(--iwe-font-ui);
  }
  .note-action:hover {
    background: var(--iwe-bg-hover);
    color: var(--iwe-text-secondary);
  }
  .note-action-delete {
    margin-left: auto;
  }
  .note-action-delete:hover {
    color: var(--iwe-danger, #b85450);
  }
</style>
