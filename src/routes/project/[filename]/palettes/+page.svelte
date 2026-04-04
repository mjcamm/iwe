<script>
  import { onMount } from 'svelte';
  import { getActiveGroups, getWordGroup, searchWordGroups, searchPaletteEntries } from '$lib/db.js';

  let groups = $state([]);
  let filteredGroups = $state([]);
  let searchHits = $state([]);
  let searchQuery = $state('');
  let selectedGroup = $state(null);
  let detail = $state(null);
  let expandedSections = $state(new Set());
  let searchInput = $state();
  let isSearching = $state(false);
  let loading = $state(true);

  onMount(async () => {
    await loadGroups();
    loading = false;
    setTimeout(() => searchInput?.focus(), 50);
  });

  async function loadGroups() {
    groups = await getActiveGroups();
    filteredGroups = groups;
    searchHits = [];
    isSearching = false;
    selectedGroup = null;
    detail = null;
  }

  async function selectGroup(g) {
    selectedGroup = g;
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

  async function handleHitGroupClick(groupId) {
    const g = groups.find(g => g.id === groupId) || filteredGroups.find(g => g.id === groupId);
    if (g) await selectGroup(g);
  }

  function entriesForSection(sectionId) {
    if (!detail) return [];
    if (sectionId === null) return detail.entries.filter(e => e.section_id === null);
    return detail.entries.filter(e => e.section_id === sectionId);
  }

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

<div class="palettes-page">
  {#if loading}
    <div class="palettes-loading">Loading palettes...</div>
  {:else}
    <div class="palettes-header">
      <h1 class="palettes-title">Word Palettes</h1>
    </div>

    <div class="palettes-search">
      <i class="bi bi-search palettes-search-icon"></i>
      <input
        bind:this={searchInput}
        class="palettes-search-input"
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

    <div class="palettes-body">
      {#if detail}
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
                  <span class="pp-pill">{entry.word}</span>
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
                    <span class="pp-pill">{entry.word}</span>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        </div>

      {:else if isSearching}
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
                      <span class="pp-hit-entry">
                        <span class="pp-hit-word">{hit.word}</span>
                        {#if hit.section_name}
                          <span class="pp-hit-section">{hit.section_name}</span>
                        {/if}
                      </span>
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
  {/if}
</div>

<style>
  .palettes-page {
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #faf8f5;
    max-width: 700px;
    margin: 0 auto;
    padding: 1.5rem;
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .palettes-loading {
    display: flex; align-items: center; justify-content: center;
    height: 50vh; color: #9e9891; font-style: italic;
  }
  .palettes-header {
    margin-bottom: 1rem;
  }
  .palettes-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.2rem; font-weight: 400; margin: 0; color: #2d2a26;
  }

  .palettes-search {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--iwe-border, #e5e1da);
    border-radius: 8px;
    background: white;
    margin-bottom: 1rem;
    flex-shrink: 0;
  }
  .palettes-search-icon { color: var(--iwe-text-faint, #c8c3bb); font-size: 0.85rem; }
  .palettes-search-input {
    flex: 1; border: none; background: none; outline: none;
    font-family: var(--iwe-font-ui, 'Source Sans 3'); font-size: 0.9rem;
    color: var(--iwe-text, #3d3a37);
  }
  .palettes-search-input::placeholder { color: var(--iwe-text-faint, #c8c3bb); }

  .palettes-body {
    flex: 1; min-height: 0; overflow-y: auto;
  }

  /* Reuse palette modal styles */
  .pp-clear-btn {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.8rem; padding: 0.2rem;
  }
  .pp-clear-btn:hover { color: var(--iwe-text); }
  .pp-back-btn {
    background: none; border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm, 4px);
    cursor: pointer; padding: 0.2rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted); display: flex; align-items: center; gap: 0.3rem;
    transition: all 100ms; white-space: nowrap;
  }
  .pp-back-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  .pp-results { padding: 0.4rem 0; }
  .pp-results-section { margin-bottom: 0.5rem; }
  .pp-results-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--iwe-text-faint, #c8c3bb); margin: 0.5rem 0.6rem 0.3rem;
  }
  .pp-result-group {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; background: none; border: none; border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
    cursor: pointer; padding: 0.6rem 0.7rem; text-align: left;
    font-family: var(--iwe-font-ui); transition: background 80ms;
  }
  .pp-result-group:last-child { border-bottom: none; }
  .pp-result-group:hover { background: var(--iwe-bg-hover, #f5f3ef); }
  .pp-result-group-name { font-size: 0.88rem; color: var(--iwe-text, #3d3a37); font-weight: 500; flex: 1; }
  .pp-result-group-count { font-size: 0.72rem; color: var(--iwe-text-faint, #c8c3bb); }
  .pp-result-arrow { font-size: 0.65rem; color: var(--iwe-text-faint); }

  .pp-hit-group {
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
    padding: 0.4rem 0;
  }
  .pp-hit-group:last-child { border-bottom: none; }
  .pp-hit-group-header {
    background: none; border: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem; font-weight: 600;
    color: var(--iwe-text-secondary, #6b6560); padding: 0.3rem 0.7rem;
    display: flex; align-items: center; gap: 0.3rem;
    transition: color 80ms; width: 100%; text-align: left;
  }
  .pp-hit-group-header:hover { color: var(--iwe-accent); }
  .pp-hit-entries { padding: 0.15rem 0.7rem 0.3rem; }
  .pp-hit-entry {
    display: flex; align-items: baseline; gap: 0.5rem;
    padding: 0.3rem 0.4rem; border-radius: var(--iwe-radius-sm, 4px);
  }
  .pp-hit-word {
    font-family: var(--iwe-font-prose, 'Libre Baskerville'); font-size: 0.88rem;
    color: var(--iwe-text, #3d3a37); line-height: 1.5;
  }
  .pp-hit-section {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-faint); text-transform: uppercase; letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .pp-browse { padding: 0.4rem 0; }
  .pp-browse-header { margin-bottom: 0.75rem; }
  .pp-browse-name {
    font-family: var(--iwe-font-prose, 'Libre Baskerville'); font-size: 1.1rem; font-weight: 600;
    margin: 0 0 0.3rem; color: var(--iwe-text, #3d3a37);
  }
  .pp-desc {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted, #9e9891); margin: 0;
    font-style: italic; line-height: 1.5;
  }
  .pp-word-section { margin-bottom: 0.75rem; }

  .pp-section {
    margin-bottom: 0.6rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
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
    font-size: 0.78rem; font-weight: 600; color: var(--iwe-text-secondary, #6b6560);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .pp-section-count { font-size: 0.68rem; color: var(--iwe-text-faint); }

  .pp-pills {
    display: flex; flex-direction: column;
    gap: 0; padding: 0.3rem 0 0.2rem 1.2rem;
  }
  .pp-pill {
    font-family: var(--iwe-font-prose, 'Libre Baskerville'); font-size: 0.88rem;
    padding: 0.5rem 0.5rem; border-radius: var(--iwe-radius-sm, 4px);
    color: var(--iwe-text-secondary, #6b6560);
    text-align: left; line-height: 1.5;
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
  }
  .pp-pill:last-child { border-bottom: none; }

  .pp-empty {
    display: flex; align-items: center; justify-content: center;
    padding: 3rem 1rem;
    color: var(--iwe-text-faint, #c8c3bb); font-style: italic;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
  }
</style>
