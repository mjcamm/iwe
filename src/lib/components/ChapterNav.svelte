<script>
  let { chapters, activeTabId, onselect, onadd, onrename, ondelete, chapterCounts = {} } = $props();

  let editingId = $state(null);
  let editTitle = $state('');

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
    if (confirm(`Delete "${ch.title}"?`)) {
      ondelete(ch.id);
    }
  }
</script>

<nav class="chapter-nav">
  <div class="nav-header">
    <span class="nav-label">Chapters</span>
    <button class="nav-add" onclick={onadd} title="Add chapter">+</button>
  </div>

  <ul class="chapter-list">
    {#each chapters as ch (ch.id)}
      <li class="chapter-item" class:active={ch.id === activeTabId}>
        {#if editingId === ch.id}
          <form class="rename-form" onsubmit={e => { e.preventDefault(); commitRename(ch.id); }}>
            <input
              class="input-author rename-input"
              bind:value={editTitle}
              onblur={() => commitRename(ch.id)}
              onkeydown={e => { if (e.key === 'Escape') editingId = null; }}
            />
          </form>
        {:else}
          <button class="chapter-btn" onclick={() => onselect(ch.id)}>
            <span class="ch-title">{ch.title}</span>
            {#if chapterCounts[ch.id]}
              <span class="ch-count" title="{chapterCounts[ch.id]} entity mentions">{chapterCounts[ch.id]}</span>
            {/if}
          </button>
          <div class="ch-actions">
            <button class="ch-action" onclick={e => startRename(e, ch)} title="Rename">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M11.5 1.5l3 3-9 9H2.5v-3z" stroke-linejoin="round"/>
              </svg>
            </button>
            <button class="ch-action ch-action-delete" onclick={e => handleDelete(e, ch)} title="Delete">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M4 4l8 8M12 4l-8 8" stroke-linecap="round"/>
              </svg>
            </button>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</nav>

<style>
  .chapter-nav { padding: 0.75rem 0; }

  .nav-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0 0.75rem; margin-bottom: 0.5rem;
  }
  .nav-label {
    font-size: 0.7rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.08em; color: var(--iwe-text-muted);
  }
  .nav-add {
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    font-size: 0.95rem; color: var(--iwe-text-muted);
    width: 22px; height: 22px; display: flex;
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
  }
  .ch-count {
    font-size: 0.65rem; color: var(--iwe-text-faint);
    flex-shrink: 0; margin-left: 0.5rem;
  }

  .ch-actions {
    display: flex; gap: 1px; opacity: 0;
    transition: opacity 100ms; position: absolute; right: 4px;
  }
  .ch-action {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); padding: 3px; display: flex;
    align-items: center; border-radius: 2px; transition: all 100ms;
  }
  .ch-action:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-active); }
  .ch-action-delete:hover { color: var(--iwe-danger); }

  .rename-form { flex: 1; padding: 0.2rem 0.4rem; }
  .rename-input {
    width: 100%; padding: 0.2rem 0.4rem; font-size: 0.85rem;
  }
</style>
