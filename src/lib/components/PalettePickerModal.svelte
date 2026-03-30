<script>
  import { getActiveGroups, getWordGroup, searchWordGroups, searchPaletteEntries } from '$lib/db.js';

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
  let searchHits = $state([]);
  let searchQuery = $state('');
  let selectedGroup = $state(null);
  let detail = $state(null);
  let expandedSections = $state(new Set());
  let searchInput = $state();
  let isSearching = $state(false);

  let lastGroupId = null;

  $effect(() => {
    if (show) {
      loadGroups();
      expandedSections = new Set();
    }
  });

  async function loadGroups() {
    groups = await getActiveGroups();

    if (word) {
      searchQuery = word;
      await handleSearch();
    } else {
      searchQuery = '';
      filteredGroups = groups;
      searchHits = [];
      isSearching = false;
      if (lastGroupId) {
        const g = groups.find(g => g.id === lastGroupId);
        if (g) { await selectGroup(g); return; }
      }
      selectedGroup = null;
      detail = null;
    }
    setTimeout(() => { searchInput?.focus(); searchInput?.select(); }, 50);
  }

  async function selectGroup(g) {
    selectedGroup = g;
    lastGroupId = g.id;
    detail = await getWordGroup(g.id);
    expandedSections = new Set();
    isSearching = false;
  }

  async function handleSearch() {
    const q = searchQuery.trim();
    if (!q) {
      filteredGroups = groups;
      searchHits = [];
      isSearching = false;
      selectedGroup = null;
      detail = null;
      return;
    }
    isSearching = true;
    selectedGroup = null;
    detail = null;
    const [groupResults, entryResults] = await Promise.all([
      searchWordGroups(q),
      searchPaletteEntries(q),
    ]);
    filteredGroups = groupResults;
    searchHits = entryResults;
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

  async function handleHitGroupClick(groupId) {
    const g = groups.find(g => g.id === groupId) || filteredGroups.find(g => g.id === groupId);
    if (g) await selectGroup(g);
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

  // Group search hits by group name for the flat results view
  let groupedHits = $derived(() => {
    const map = new Map();
    for (const hit of searchHits) {
      if (!map.has(hit.group_id)) {
        map.set(hit.group_id, { group_id: hit.group_id, group_name: hit.group_name, hits: [] });
      }
      map.get(hit.group_id).hits.push(hit);
    }
    return [...map.values()];
  });
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
          placeholder="Search groups and words..."
          bind:value={searchQuery}
          oninput={handleSearch}
        />
        {#if isSearching && !detail}
          <button class="pp-clear-btn" onclick={() => { searchQuery = ''; handleSearch(); }}>
            <i class="bi bi-x-lg"></i>
          </button>
        {/if}
        {#if detail}
          <button class="pp-back-btn" onclick={() => { detail = null; selectedGroup = null; isSearching = !!searchQuery.trim(); }}>
            <i class="bi bi-arrow-left"></i> Back
          </button>
        {/if}
      </div>

      <div class="pp-body">
        {#if detail}
          <!-- Browse view: full group detail -->
          <div class="pp-browse">
            <div class="pp-browse-header">
              <h3 class="pp-browse-name">{detail.group.name}</h3>
              {#if detail.group.description}
                <p class="pp-desc">{detail.group.description}</p>
              {/if}
            </div>

            {#if entriesForSection(null).length > 0}
              <div class="pp-word-section">
                <div class="pp-pills">
                  {#each entriesForSection(null) as entry (entry.id)}
                    <button class="pp-pill" onclick={() => handleReplace(entry.word)}>{entry.word}</button>
                  {/each}
                </div>
              </div>
            {/if}

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
          </div>

        {:else if isSearching}
          <!-- Search results view: flat list grouped by group -->
          <div class="pp-results">
            {#if filteredGroups.length > 0}
              <div class="pp-results-section">
                <p class="pp-results-label">Groups</p>
                {#each filteredGroups as g (g.id)}
                  <button class="pp-result-group" onclick={() => selectGroup(g)}>
                    <span class="pp-result-group-name">{g.name}</span>
                    <span class="pp-result-group-count">{g.entry_count} words</span>
                    <i class="bi bi-chevron-right pp-result-arrow"></i>
                  </button>
                {/each}
              </div>
            {/if}

            {#if groupedHits().length > 0}
              <div class="pp-results-section">
                <p class="pp-results-label">Matching words</p>
                {#each groupedHits() as gh (gh.group_id)}
                  <div class="pp-hit-group">
                    <button class="pp-hit-group-header" onclick={() => handleHitGroupClick(gh.group_id)}>
                      {gh.group_name} <i class="bi bi-chevron-right pp-result-arrow"></i>
                    </button>
                    <div class="pp-hit-entries">
                      {#each gh.hits as hit (hit.entry_id)}
                        <button class="pp-hit-entry" onclick={() => handleReplace(hit.word)}>
                          <span class="pp-hit-word">{hit.word}</span>
                          {#if hit.section_name}
                            <span class="pp-hit-section">{hit.section_name}</span>
                          {/if}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}

            {#if filteredGroups.length === 0 && groupedHits().length === 0}
              <div class="pp-empty">No results for "{searchQuery}"</div>
            {/if}
          </div>

        {:else}
          <!-- Default: group list -->
          <div class="pp-results">
            {#each groups as g (g.id)}
              <button class="pp-result-group" onclick={() => selectGroup(g)}>
                <span class="pp-result-group-name">{g.name}</span>
                <span class="pp-result-group-count">{g.entry_count} words</span>
                <i class="bi bi-chevron-right pp-result-arrow"></i>
              </button>
            {/each}
          </div>
        {/if}
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
    width: 90vw; max-width: 640px;
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
  .pp-clear-btn {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.8rem; padding: 0.2rem;
  }
  .pp-clear-btn:hover { color: var(--iwe-text); }
  .pp-back-btn {
    background: none; border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.2rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted); display: flex; align-items: center; gap: 0.3rem;
    transition: all 100ms; white-space: nowrap;
  }
  .pp-back-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  .pp-body {
    flex: 1; min-height: 0; overflow-y: auto;
  }

  /* Results list (search results + default group list) */
  .pp-results { padding: 0.4rem; }
  .pp-results-section { margin-bottom: 0.5rem; }
  .pp-results-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--iwe-text-faint); margin: 0.5rem 0.6rem 0.3rem;
  }
  .pp-result-group {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; background: none; border: none; border-bottom: 1px solid var(--iwe-border-light);
    cursor: pointer; padding: 0.6rem 0.7rem; text-align: left;
    font-family: var(--iwe-font-ui); transition: background 80ms;
  }
  .pp-result-group:last-child { border-bottom: none; }
  .pp-result-group:hover { background: var(--iwe-bg-hover); }
  .pp-result-group-name { font-size: 0.88rem; color: var(--iwe-text); font-weight: 500; flex: 1; }
  .pp-result-group-count { font-size: 0.72rem; color: var(--iwe-text-faint); }
  .pp-result-arrow { font-size: 0.65rem; color: var(--iwe-text-faint); }

  /* Search hit groups */
  .pp-hit-group {
    border-bottom: 1px solid var(--iwe-border-light);
    padding: 0.4rem 0;
  }
  .pp-hit-group:last-child { border-bottom: none; }
  .pp-hit-group-header {
    background: none; border: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem; font-weight: 600;
    color: var(--iwe-text-secondary); padding: 0.3rem 0.7rem;
    display: flex; align-items: center; gap: 0.3rem;
    transition: color 80ms; width: 100%; text-align: left;
  }
  .pp-hit-group-header:hover { color: var(--iwe-accent); }
  .pp-hit-entries { padding: 0.15rem 0.7rem 0.3rem; }
  .pp-hit-entry {
    display: flex; align-items: baseline; gap: 0.5rem;
    width: 100%; background: none; border: none; cursor: pointer;
    padding: 0.3rem 0.4rem; text-align: left;
    border-radius: var(--iwe-radius-sm);
    transition: background 80ms;
  }
  .pp-hit-entry:hover { background: var(--iwe-accent-light); }
  .pp-hit-word {
    font-family: var(--iwe-font-prose); font-size: 0.88rem;
    color: var(--iwe-text); line-height: 1.5;
  }
  .pp-hit-entry:hover .pp-hit-word { color: var(--iwe-accent); }
  .pp-hit-section {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-faint); text-transform: uppercase; letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  /* Browse view (group detail) */
  .pp-browse { padding: 0.8rem 1.1rem; }
  .pp-browse-header { margin-bottom: 0.75rem; }
  .pp-browse-name {
    font-family: var(--iwe-font-prose); font-size: 1.1rem; font-weight: 600;
    margin: 0 0 0.3rem; color: var(--iwe-text);
  }
  .pp-desc {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); margin: 0;
    font-style: italic; line-height: 1.5;
  }
  .pp-word-section { margin-bottom: 0.75rem; }

  .pp-section {
    margin-bottom: 0.6rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .pp-section:last-child { border-bottom: none; }
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
    display: flex; flex-direction: column;
    gap: 0; padding: 0.3rem 0 0.2rem 1.2rem;
  }
  .pp-pill {
    font-family: var(--iwe-font-prose); font-size: 0.88rem;
    padding: 0.5rem 0.5rem; border-radius: var(--iwe-radius-sm);
    cursor: pointer;
    border: none; background: none;
    color: var(--iwe-text-secondary);
    transition: all 100ms;
    text-align: left; line-height: 1.5;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .pp-pill:last-child { border-bottom: none; }
  .pp-pill:hover { color: var(--iwe-accent); background: var(--iwe-accent-light); }

  .pp-empty {
    display: flex; align-items: center; justify-content: center;
    padding: 3rem 1rem;
    color: var(--iwe-text-faint); font-style: italic;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
  }
</style>
