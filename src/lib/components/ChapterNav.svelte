<script>
  import { getDeletedChapters, restoreChapter } from '$lib/db.js';

  let { chapters, activeTabId, onselect, onadd, onrename, ondelete, onrestore } = $props();

  let editingId = $state(null);
  let editTitle = $state('');
  let confirmDeleteId = $state(null);

  // Deleted chapters modal
  let showDeleted = $state(false);
  let deletedChapters = $state([]);
  let loadingDeleted = $state(false);

  function startRename(e, ch) {
    e.stopPropagation();
    editingId = ch.id;
    editTitle = ch.title;
  }

  function commitRename(id) {
    if (editTitle.trim() && editTitle.trim() !== chapters.find(c => c.id === id)?.title) {
      onrename(id, editTitle.trim());
    }
    editingId = null;
  }

  function handleDelete(e, ch) {
    e.stopPropagation();
    if (confirmDeleteId !== ch.id) {
      confirmDeleteId = ch.id;
      return;
    }
    confirmDeleteId = null;
    ondelete(ch.id);
  }

  function cancelDelete(e) {
    e.stopPropagation();
    confirmDeleteId = null;
  }

  async function openDeletedModal() {
    showDeleted = true;
    loadingDeleted = true;
    try {
      deletedChapters = await getDeletedChapters();
    } catch (e) {
      console.error('[chapters] load deleted failed:', e);
      deletedChapters = [];
    }
    loadingDeleted = false;
  }

  async function handleRestore(id) {
    await restoreChapter(id);
    deletedChapters = deletedChapters.filter(c => c.id !== id);
    onrestore?.();
  }
</script>

<nav class="chapter-nav">
  <div class="nav-header">
    <span class="nav-label">Chapters</span>
    <div class="nav-header-actions">
      <button class="nav-icon-btn" onclick={openDeletedModal} title="Show deleted chapters">
        <i class="bi bi-archive"></i>
      </button>
      <button class="nav-add" onclick={onadd} title="Add chapter">+</button>
    </div>
  </div>

  <ul class="chapter-list">
    {#each chapters as ch (ch.id)}
      <li class="chapter-item" class:active={ch.id === activeTabId}>
        {#if editingId === ch.id}
          <form class="rename-form" onsubmit={e => { e.preventDefault(); commitRename(ch.id); }}>
            <input
              class="input-author rename-input"
              bind:value={editTitle}
              onkeydown={e => { if (e.key === 'Escape') editingId = null; }}
            />
            <button type="submit" class="ch-action ch-action-save" title="Save">
              <i class="bi bi-check-lg"></i>
            </button>
            <button type="button" class="ch-action" onclick={() => editingId = null} title="Cancel">
              <i class="bi bi-x-lg"></i>
            </button>
          </form>
        {:else}
          <button class="chapter-btn" onclick={() => onselect(ch.id)}>
            <span class="ch-title">{ch.title}</span>
          </button>
          <div class="ch-actions">
            <button class="ch-action" onclick={e => startRename(e, ch)} title="Rename">
              <i class="bi bi-pencil"></i>
            </button>
            {#if confirmDeleteId === ch.id}
              <button class="ch-action ch-action-confirm" onclick={e => handleDelete(e, ch)} title="Confirm delete">
                <i class="bi bi-check-lg"></i>
              </button>
              <button class="ch-action" onclick={cancelDelete} title="Cancel">
                <i class="bi bi-x-lg"></i>
              </button>
            {:else}
              <button class="ch-action ch-action-delete" onclick={e => handleDelete(e, ch)} title="Delete">
                <i class="bi bi-trash3"></i>
              </button>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</nav>

<!-- Deleted chapters modal -->
{#if showDeleted}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="deleted-backdrop" onclick={e => { if (e.target === e.currentTarget) showDeleted = false; }}>
    <div class="deleted-modal">
      <div class="deleted-header">
        <span class="deleted-title">Deleted Chapters</span>
        <button class="deleted-close" onclick={() => showDeleted = false}>&times;</button>
      </div>
      <div class="deleted-body">
        {#if loadingDeleted}
          <p class="deleted-empty">Loading...</p>
        {:else if deletedChapters.length === 0}
          <p class="deleted-empty">No deleted chapters</p>
        {:else}
          {#each deletedChapters as ch (ch.id)}
            <div class="deleted-item">
              <span class="deleted-item-title">{ch.title}</span>
              <button class="deleted-restore" onclick={() => handleRestore(ch.id)}>
                <i class="bi bi-arrow-counterclockwise"></i> Restore
              </button>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<svelte:window onkeydown={e => { if (e.key === 'Escape' && showDeleted) showDeleted = false; }} />

<style>
  .chapter-nav { padding: 0.75rem 0; }

  .nav-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0 0.75rem; margin-bottom: 0.5rem;
  }
  .nav-header-actions { display: flex; align-items: center; gap: 0.3rem; }
  .nav-label {
    font-size: 1rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.08em; color: var(--iwe-text-muted);
  }
  .nav-icon-btn {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.85rem;
    padding: 0.15rem 0.3rem; border-radius: var(--iwe-radius-sm);
    display: flex; align-items: center; transition: all 150ms;
  }
  .nav-icon-btn:hover { color: var(--iwe-accent); background: var(--iwe-bg-hover); }
  .nav-add {
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    font-size: 1.3rem; color: var(--iwe-text-muted);
    width: 25px; height: 25px; display: flex;
    align-items: center; justify-content: center;
    padding: 0; line-height: 1; transition: all 150ms;
  }
  .nav-add:hover {
    background: var(--iwe-bg-hover); color: var(--iwe-accent);
    border-color: var(--iwe-accent);
  }

  .chapter-list { list-style: none; padding: 0; margin: 0; }

  .chapter-item {
    display: flex; align-items: center; position: relative;
    margin: 0 0.4rem; border-radius: var(--iwe-radius-sm);
  }
  .chapter-item:hover { background: var(--iwe-bg-hover); }
  .chapter-item:hover .ch-actions { opacity: 1; }

  .chapter-item.active {
    background: var(--iwe-bg-active);
  }
  .chapter-item.active .ch-title { color: var(--iwe-text); font-weight: 500; }

  .chapter-btn {
    flex: 1; display: flex; align-items: center; justify-content: space-between;
    background: none; border: none; padding: 0.4rem 0.5rem;
    cursor: pointer; text-align: left; color: inherit;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    min-width: 0;
  }
  .ch-title {
    color: var(--iwe-text-secondary); white-space: nowrap;
    overflow: hidden; text-overflow: ellipsis;
    transition: color 150ms;
    font-size: 1rem;
  }

  .ch-actions {
    display: flex; gap: 1px; opacity: 0;
    transition: opacity 100ms; position: absolute; right: 4px;
  }
  .ch-action {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); padding: 4px; display: flex;
    align-items: center; border-radius: 3px; transition: all 100ms;
    font-size: 0.95rem;
  }
  .ch-action:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-active); }
  .ch-action-delete:hover { color: var(--iwe-danger, #b85450); }
  .ch-action-confirm { color: var(--iwe-danger, #b85450); }
  .ch-action-confirm:hover { color: white; background: var(--iwe-danger, #b85450); }

  .rename-form { flex: 1; padding: 0.2rem 0.4rem; display: flex; align-items: center; gap: 2px; }
  .rename-input {
    flex: 1; padding: 0.2rem 0.4rem; font-size: 0.9rem;
  }
  .ch-action-save { color: var(--iwe-accent, #2d6a5e); }
  .ch-action-save:hover { color: white; background: var(--iwe-accent, #2d6a5e); }

  /* Deleted chapters modal */
  .deleted-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.35);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 10vh;
  }
  .deleted-modal {
    background: var(--iwe-bg, white); border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2);
    width: 90vw; max-width: 420px;
    animation: del-slide 0.2s ease;
  }
  @keyframes del-slide {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .deleted-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.8rem 1.2rem;
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
  }
  .deleted-title {
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    font-weight: 600; color: var(--iwe-text);
  }
  .deleted-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.4rem; line-height: 1; color: var(--iwe-text-faint);
  }
  .deleted-close:hover { color: var(--iwe-text); }
  .deleted-body {
    padding: 0.5rem 0.8rem 1rem;
    max-height: 50vh; overflow-y: auto;
  }
  .deleted-empty {
    text-align: center; color: var(--iwe-text-faint);
    font-style: italic; font-size: 0.85rem;
    padding: 1.5rem 0;
  }
  .deleted-item {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.5rem 0.4rem;
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
  }
  .deleted-item:last-child { border-bottom: none; }
  .deleted-item-title {
    font-family: var(--iwe-font-ui); font-size: 0.88rem;
    color: var(--iwe-text);
  }
  .deleted-restore {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.25rem 0.6rem; border: 1px solid var(--iwe-accent, #2d6a5e);
    border-radius: 4px; cursor: pointer;
    background: none; color: var(--iwe-accent, #2d6a5e);
    display: flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .deleted-restore:hover { background: var(--iwe-accent, #2d6a5e); color: white; }
</style>
