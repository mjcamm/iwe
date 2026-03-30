<script>
  import { getActiveGroups, getWordGroup, searchWordGroups } from '$lib/db.js';

  let {
    show = false,
    word = '',
    editorFrom = 0,
    editorTo = 0,
    onreplace,
    onclose,
  } = $props();

  let groups = $state([]);
  let filteredGroups = $state([]);
  let searchQuery = $state('');
  let selectedGroup = $state(null);
  let detail = $state(null);
  let expandedSections = $state(new Set());
  let searchInput = $state();

  // Remember last selected group within session
  let lastGroupId = null;

  $effect(() => {
    if (show) {
      loadGroups();
      searchQuery = '';
      expandedSections = new Set();
    }
  });

  async function loadGroups() {
    groups = await getActiveGroups();
    filteredGroups = groups;
    // Restore last group or auto-select first
    if (lastGroupId) {
      const g = groups.find(g => g.id === lastGroupId);
      if (g) { await selectGroup(g); return; }
    }
    if (groups.length === 1) {
      await selectGroup(groups[0]);
    } else {
      selectedGroup = null;
      detail = null;
    }
    // Focus search after a tick
    setTimeout(() => searchInput?.focus(), 50);
  }

  async function selectGroup(g) {
    selectedGroup = g;
    lastGroupId = g.id;
    detail = await getWordGroup(g.id);
    expandedSections = new Set();
    // Auto-expand if flat (no sections) or only unsectioned words
    if (detail.sections.length === 0) {
      // flat group — no sections to expand
    }
  }

  async function handleSearch() {
    if (!searchQuery.trim()) {
      filteredGroups = groups;
    } else {
      filteredGroups = await searchWordGroups(searchQuery.trim());
    }
    // Auto-select if one match
    if (filteredGroups.length === 1 && (!selectedGroup || selectedGroup.id !== filteredGroups[0].id)) {
      await selectGroup(filteredGroups[0]);
    }
  }

  function toggleSection(secId) {
    const next = new Set(expandedSections);
    if (next.has(secId)) next.delete(secId);
    else next.add(secId);
    expandedSections = next;
  }

  function handleReplace(newWord) {
    onreplace?.(newWord, editorFrom, editorTo);
    onclose?.();
  }

  function handleClose() {
    onclose?.();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') handleClose();
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) handleClose();
  }

  function entriesForSection(sectionId) {
    if (!detail) return [];
    if (sectionId === null) return detail.entries.filter(e => e.section_id === null);
    return detail.entries.filter(e => e.section_id === sectionId);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="pp-backdrop" onclick={handleBackdropClick}>
    <div class="pp-modal" role="dialog" aria-modal="true">
      <div class="pp-header">
        <div class="pp-header-text">
          <span class="pp-header-title">Word Palettes</span>
          {#if word}
            <span class="pp-header-replacing">Replacing: "{word}"</span>
          {/if}
        </div>
        <button class="pp-close" onclick={handleClose}>&times;</button>
      </div>

      <div class="pp-search">
        <i class="bi bi-search pp-search-icon"></i>
        <input
          bind:this={searchInput}
          class="pp-search-input"
          type="text"
          placeholder="Search groups..."
          bind:value={searchQuery}
          oninput={handleSearch}
        />
      </div>

      <div class="pp-body">
        <div class="pp-group-list">
          {#each filteredGroups as g (g.id)}
            <button
              class="pp-group-item"
              class:active={selectedGroup?.id === g.id}
              onclick={() => selectGroup(g)}
            >
              <span class="pp-group-name">{g.name}</span>
              <span class="pp-group-count">{g.entry_count} words</span>
            </button>
          {/each}
          {#if filteredGroups.length === 0}
            <p class="pp-no-results">No matching groups</p>
          {/if}
        </div>

        <div class="pp-group-detail">
          {#if detail}
            {#if detail.group.description}
              <p class="pp-desc">{detail.group.description}</p>
            {/if}

            <!-- Unsectioned words (flat groups or orphaned words) -->
            {#if entriesForSection(null).length > 0}
              <div class="pp-word-section">
                <div class="pp-pills">
                  {#each entriesForSection(null) as entry (entry.id)}
                    <button class="pp-pill" onclick={() => handleReplace(entry.word)}>{entry.word}</button>
                  {/each}
                </div>
              </div>
            {/if}

            <!-- Sections -->
            {#each detail.sections as sec (sec.id)}
              <div class="pp-section">
                <button class="pp-section-toggle" onclick={() => toggleSection(sec.id)}>
                  <i class="bi" class:bi-chevron-down={expandedSections.has(sec.id)} class:bi-chevron-right={!expandedSections.has(sec.id)}></i>
                  <span class="pp-section-name">{sec.name}</span>
                  <span class="pp-section-count">({entriesForSection(sec.id).length})</span>
                </button>
                {#if expandedSections.has(sec.id)}
                  <div class="pp-pills">
                    {#each entriesForSection(sec.id) as entry (entry.id)}
                      <button class="pp-pill" onclick={() => handleReplace(entry.word)}>{entry.word}</button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="pp-empty">Select a group to browse words</div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .pp-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.35);
    display: flex; align-items: center; justify-content: center;
    animation: pp-fade 0.15s ease;
  }
  @keyframes pp-fade { from { opacity: 0; } to { opacity: 1; } }

  .pp-modal {
    background: var(--iwe-bg, #fff);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2), 0 4px 16px rgba(0,0,0,0.1);
    width: 90vw; max-width: 700px;
    max-height: 80vh;
    display: flex; flex-direction: column;
    animation: pp-slide 0.2s ease;
    overflow: hidden;
  }
  @keyframes pp-slide {
    from { opacity: 0; transform: translateY(12px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  .pp-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1rem 1.3rem;
    border-bottom: 1px solid var(--iwe-border, #e5e1da);
    flex-shrink: 0;
  }
  .pp-header-text { display: flex; flex-direction: column; gap: 0.15rem; }
  .pp-header-title {
    font-family: var(--iwe-font-ui); font-size: 1rem; font-weight: 600;
    color: var(--iwe-text);
  }
  .pp-header-replacing {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); font-style: italic;
  }
  .pp-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.6rem; line-height: 1; padding: 0.2rem 0.4rem;
    color: var(--iwe-text-faint); border-radius: var(--iwe-radius-sm);
  }
  .pp-close:hover { color: var(--iwe-text); background: var(--iwe-bg-hover); }

  .pp-search {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.6rem 1.3rem;
    border-bottom: 1px solid var(--iwe-border-light);
    flex-shrink: 0;
  }
  .pp-search-icon { color: var(--iwe-text-faint); font-size: 0.85rem; }
  .pp-search-input {
    flex: 1; border: none; background: none; outline: none;
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    color: var(--iwe-text);
  }
  .pp-search-input::placeholder { color: var(--iwe-text-faint); }

  .pp-body {
    display: flex; flex: 1; min-height: 0; overflow: hidden;
  }

  .pp-group-list {
    width: 180px; flex-shrink: 0; overflow-y: auto;
    border-right: 1px solid var(--iwe-border-light);
    padding: 0.35rem;
  }
  .pp-group-item {
    display: flex; flex-direction: column; gap: 0.1rem;
    width: 100%; background: none; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.4rem 0.6rem; text-align: left;
    font-family: var(--iwe-font-ui); transition: background 80ms;
  }
  .pp-group-item:hover { background: var(--iwe-bg-hover); }
  .pp-group-item.active { background: var(--iwe-accent-light); }
  .pp-group-name { font-size: 0.82rem; color: var(--iwe-text); font-weight: 500; }
  .pp-group-item.active .pp-group-name { color: var(--iwe-accent); }
  .pp-group-count { font-size: 0.68rem; color: var(--iwe-text-faint); }
  .pp-no-results {
    font-size: 0.8rem; color: var(--iwe-text-faint); text-align: center;
    padding: 1rem; font-style: italic;
  }

  .pp-group-detail {
    flex: 1; overflow-y: auto; padding: 0.8rem 1.1rem;
  }
  .pp-desc {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); margin: 0 0 0.75rem;
    font-style: italic; line-height: 1.5;
  }
  .pp-word-section { margin-bottom: 0.75rem; }

  .pp-section { margin-bottom: 0.5rem; }
  .pp-section-toggle {
    display: flex; align-items: center; gap: 0.4rem;
    width: 100%; background: none; border: none; cursor: pointer;
    padding: 0.35rem 0; text-align: left;
    font-family: var(--iwe-font-ui);
    transition: color 80ms;
  }
  .pp-section-toggle:hover { color: var(--iwe-accent); }
  .pp-section-toggle i { font-size: 0.7rem; color: var(--iwe-text-faint); width: 0.8rem; }
  .pp-section-name {
    font-size: 0.78rem; font-weight: 600; color: var(--iwe-text-secondary);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .pp-section-count { font-size: 0.68rem; color: var(--iwe-text-faint); }

  .pp-pills {
    display: flex; flex-wrap: wrap;
    gap: 0; line-height: 1.9;
    padding: 0.3rem 0 0.2rem 1.2rem;
  }
  .pp-pill {
    font-family: var(--iwe-font-prose); font-size: 0.88rem;
    padding: 0; border-radius: 3px;
    cursor: pointer;
    border: none; background: none;
    color: var(--iwe-text-secondary);
    transition: color 100ms;
  }
  .pp-pill::after {
    content: '\b7';
    color: var(--iwe-text-faint);
    margin: 0 0.45rem;
    font-weight: 300;
  }
  .pp-pill:last-child::after { content: ''; margin: 0; }
  .pp-pill:hover {
    color: var(--iwe-accent);
  }

  .pp-empty {
    display: flex; align-items: center; justify-content: center;
    height: 100%; color: var(--iwe-text-faint); font-style: italic;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
  }
</style>
