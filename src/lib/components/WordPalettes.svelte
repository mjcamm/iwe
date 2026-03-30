<script>
  import { onMount } from 'svelte';
  import { getPalettes, createPalette, updatePalette, deletePalette, togglePalette, copyPalette, getWordGroups, getWordGroup, createWordGroup, updateWordGroup, deleteWordGroup, getAllSectionNames, addSection, addAllSections, renameSection, deleteSection, addWordEntries, removeWordEntry } from '$lib/db.js';

  let palettes = $state([]);
  let selectedPaletteId = $state(null);
  let selectedPalette = $state(null);
  let groups = $state([]);
  let selectedGroupId = $state(null);
  let detail = $state(null);
  let loading = $state(true);

  // Palette forms
  let showNewPalette = $state(false);
  let newPaletteName = $state('');
  let showCopyDialog = $state(false);
  let copySourceId = $state(null);
  let copyName = $state('');

  // Group forms
  let showNewGroup = $state(false);
  let newGroupName = $state('');

  // Edit description
  let editingDesc = $state(false);
  let descDraft = $state('');

  // Add section
  let showAddSection = $state(false);
  let allSectionNames = $state([]);
  let newSectionName = $state('');

  // Rename section
  let renamingSectionId = $state(null);
  let renamingSectionName = $state('');

  // Bulk add
  let bulkText = $state('');
  let bulkSectionId = $state('null');

  onMount(async () => {
    await refreshPalettes();
    loading = false;
  });

  async function refreshPalettes() {
    palettes = await getPalettes();
    if (selectedPaletteId) {
      selectedPalette = palettes.find(p => p.id === selectedPaletteId) || null;
      if (selectedPalette) await refreshGroups();
      else { selectedPaletteId = null; groups = []; detail = null; }
    }
  }

  async function refreshGroups() {
    if (!selectedPaletteId) return;
    groups = await getWordGroups(selectedPaletteId);
    if (selectedGroupId) await loadGroupDetail(selectedGroupId);
  }

  async function selectPalette(p) {
    selectedPaletteId = p.id;
    selectedPalette = p;
    selectedGroupId = null;
    detail = null;
    groups = await getWordGroups(p.id);
    showNewGroup = false;
    editingDesc = false;
    showAddSection = false;
  }

  async function loadGroupDetail(id) {
    selectedGroupId = id;
    detail = await getWordGroup(id);
    editingDesc = false;
    showAddSection = false;
  }

  // Palette CRUD
  async function handleCreatePalette() {
    if (!newPaletteName.trim()) return;
    const p = await createPalette(newPaletteName.trim());
    newPaletteName = '';
    showNewPalette = false;
    await refreshPalettes();
    await selectPalette(p);
  }

  async function handleToggle(p) {
    await togglePalette(p.id, !p.is_active);
    palettes = await getPalettes();
    if (selectedPaletteId === p.id) selectedPalette = palettes.find(x => x.id === p.id);
  }

  async function handleDeletePalette(p) {
    if (p.is_system) return;
    if (!confirm(`Delete "${p.name}" and all its groups/words?`)) return;
    await deletePalette(p.id);
    if (selectedPaletteId === p.id) { selectedPaletteId = null; selectedPalette = null; groups = []; detail = null; }
    palettes = await getPalettes();
  }

  function startCopy(p) {
    copySourceId = p.id;
    copyName = p.name + ' (Copy)';
    showCopyDialog = true;
  }

  async function handleCopy() {
    if (!copyName.trim()) return;
    const p = await copyPalette(copySourceId, copyName.trim());
    showCopyDialog = false;
    await refreshPalettes();
    await selectPalette(p);
  }

  // Group CRUD
  async function handleCreateGroup() {
    if (!newGroupName.trim() || !selectedPaletteId) return;
    const g = await createWordGroup(selectedPaletteId, newGroupName.trim());
    newGroupName = '';
    showNewGroup = false;
    await refreshGroups();
    await loadGroupDetail(g.id);
  }

  async function handleDeleteGroup() {
    if (!detail) return;
    if (!confirm(`Delete "${detail.group.name}" and all ${detail.group.entry_count} words?`)) return;
    await deleteWordGroup(detail.group.id);
    selectedGroupId = null;
    detail = null;
    await refreshGroups();
    palettes = await getPalettes();
  }

  async function handleSaveDesc() {
    if (!detail) return;
    await updateWordGroup(detail.group.id, detail.group.name, descDraft.trim() || null);
    editingDesc = false;
    await loadGroupDetail(detail.group.id);
  }

  async function handleRenameGroup(e) {
    if (!detail) return;
    const name = e.target.textContent.trim();
    if (name && name !== detail.group.name) {
      await updateWordGroup(detail.group.id, name, detail.group.description);
      await refreshGroups();
    }
  }

  // Sections
  async function openAddSection() {
    allSectionNames = await getAllSectionNames();
    showAddSection = true;
    newSectionName = '';
  }

  function existingSectionNames() {
    if (!detail) return new Set();
    return new Set(detail.sections.map(s => s.name));
  }

  async function handleAddSection(name) {
    if (!detail) return;
    await addSection(detail.group.id, name);
    await loadGroupDetail(detail.group.id);
  }

  async function handleAddAllSections() {
    if (!detail) return;
    await addAllSections(detail.group.id, allSectionNames);
    showAddSection = false;
    await loadGroupDetail(detail.group.id);
  }

  async function handleNewSection() {
    if (!newSectionName.trim()) return;
    await handleAddSection(newSectionName.trim());
    newSectionName = '';
  }

  async function handleDeleteSection(secId) {
    await deleteSection(secId);
    await loadGroupDetail(detail.group.id);
  }

  async function handleStartRenameSection(sec) {
    renamingSectionId = sec.id;
    renamingSectionName = sec.name;
  }

  async function handleFinishRenameSection() {
    if (renamingSectionId && renamingSectionName.trim()) {
      await renameSection(renamingSectionId, renamingSectionName.trim());
      await loadGroupDetail(detail.group.id);
    }
    renamingSectionId = null;
  }

  async function handleRemoveWord(entryId) {
    await removeWordEntry(entryId);
    await loadGroupDetail(detail.group.id);
    await refreshGroups();
    palettes = await getPalettes();
  }

  async function handleBulkAdd() {
    if (!bulkText.trim() || !detail) return;
    const words = bulkText.split(/\n+/).map(w => w.trim()).filter(w => w);
    if (words.length === 0) return;
    const secId = bulkSectionId === 'null' ? null : parseInt(bulkSectionId);
    await addWordEntries(detail.group.id, secId, words);
    bulkText = '';
    await loadGroupDetail(detail.group.id);
    await refreshGroups();
    palettes = await getPalettes();
  }

  function entriesForSection(sectionId) {
    if (!detail) return [];
    if (sectionId === null) return detail.entries.filter(e => e.section_id === null);
    return detail.entries.filter(e => e.section_id === sectionId);
  }

  let isSystemSelected = $derived(selectedPalette?.is_system || false);
</script>

<div class="wp">
  <div class="wp-header">
    <h2 class="wp-title">Word Palettes</h2>
  </div>

  {#if loading}
    <p class="wp-loading">Loading palettes...</p>
  {:else}
    <!-- Palette list (top bar) -->
    <div class="wp-palette-bar">
      <div class="wp-palette-list">
        {#each palettes as p (p.id)}
          <div class="wp-palette-chip" class:active={selectedPaletteId === p.id} class:system={p.is_system}>
            <button class="wp-palette-toggle" onclick={() => handleToggle(p)} title={p.is_active ? 'Active — click to disable' : 'Inactive — click to enable'}>
              <i class="bi" class:bi-check-circle-fill={p.is_active} class:bi-circle={!p.is_active}></i>
            </button>
            <button class="wp-palette-name" onclick={() => selectPalette(p)}>
              {p.name}
              <span class="wp-palette-meta">{p.entry_count}</span>
            </button>
            {#if p.is_system}
              <span class="wp-system-badge" title="System default — read only">
                <i class="bi bi-lock-fill"></i>
              </span>
            {/if}
            <button class="wp-palette-action" onclick={() => startCopy(p)} title="Copy palette">
              <i class="bi bi-copy"></i>
            </button>
            {#if !p.is_system}
              <button class="wp-palette-action wp-palette-delete" onclick={() => handleDeletePalette(p)} title="Delete palette">
                <i class="bi bi-trash"></i>
              </button>
            {/if}
          </div>
        {/each}
      </div>
      {#if !showNewPalette}
        <button class="wp-new-palette-btn" onclick={() => showNewPalette = true}>+ New Palette</button>
      {:else}
        <form class="wp-new-palette-form" onsubmit={e => { e.preventDefault(); handleCreatePalette(); }}>
          <input class="input-author" bind:value={newPaletteName} placeholder="Palette name..." />
          <button class="btn-author btn-author-primary btn-author-sm" type="submit">Create</button>
          <button class="btn-author btn-author-subtle btn-author-sm" type="button" onclick={() => { showNewPalette = false; newPaletteName = ''; }}>Cancel</button>
        </form>
      {/if}
    </div>

    <!-- Copy dialog -->
    {#if showCopyDialog}
      <div class="wp-copy-dialog">
        <form onsubmit={e => { e.preventDefault(); handleCopy(); }}>
          <label class="wp-copy-label">Copy as:</label>
          <input class="input-author" bind:value={copyName} />
          <button class="btn-author btn-author-primary btn-author-sm" type="submit">Copy</button>
          <button class="btn-author btn-author-subtle btn-author-sm" type="button" onclick={() => showCopyDialog = false}>Cancel</button>
        </form>
      </div>
    {/if}

    <!-- Group + detail panels -->
    {#if selectedPalette}
      <div class="wp-panels">
        <div class="wp-list">
          {#each groups as g (g.id)}
            <button class="wp-group-item" class:active={selectedGroupId === g.id} onclick={() => loadGroupDetail(g.id)}>
              <span class="wp-group-name">{g.name}</span>
              <span class="wp-group-count">({g.entry_count})</span>
            </button>
          {/each}
          {#if groups.length === 0}
            <p class="wp-empty-list">No groups yet.</p>
          {/if}
          {#if !isSystemSelected}
            {#if !showNewGroup}
              <button class="wp-add-group-btn" onclick={() => showNewGroup = true}>+ Add Group</button>
            {:else}
              <form class="wp-new-group-form" onsubmit={e => { e.preventDefault(); handleCreateGroup(); }}>
                <input class="input-author wp-new-group-input" bind:value={newGroupName} placeholder="Group name..." />
                <div class="wp-new-group-actions">
                  <button class="btn-author btn-author-primary btn-author-sm" type="submit">Add</button>
                  <button class="btn-author btn-author-subtle btn-author-sm" type="button" onclick={() => { showNewGroup = false; newGroupName = ''; }}>Cancel</button>
                </div>
              </form>
            {/if}
          {/if}
        </div>

        <div class="wp-detail">
          {#if detail}
            <div class="wp-detail-header">
              {#if isSystemSelected}
                <h3 class="wp-detail-name">{detail.group.name}</h3>
              {:else}
                <h3
                  class="wp-detail-name wp-detail-name-editable"
                  contenteditable="true"
                  onblur={handleRenameGroup}
                  onkeydown={e => { if (e.key === 'Enter') { e.preventDefault(); e.target.blur(); }}}
                >{detail.group.name}</h3>
                <button class="wp-icon-btn" onclick={handleDeleteGroup} title="Delete group"><i class="bi bi-trash"></i></button>
              {/if}
            </div>

            {#if !isSystemSelected && editingDesc}
              <div class="wp-desc-edit">
                <textarea class="wp-desc-textarea" bind:value={descDraft} rows="2" placeholder="Description..."></textarea>
                <div class="wp-desc-actions">
                  <button class="btn-author btn-author-primary btn-author-sm" onclick={handleSaveDesc}>Save</button>
                  <button class="btn-author btn-author-subtle btn-author-sm" onclick={() => editingDesc = false}>Cancel</button>
                </div>
              </div>
            {:else if detail.group.description}
              <p class="wp-desc" class:wp-desc-clickable={!isSystemSelected} onclick={() => { if (!isSystemSelected) { editingDesc = true; descDraft = detail.group.description || ''; } }}>{detail.group.description}</p>
            {:else if !isSystemSelected}
              <button class="wp-desc-add" onclick={() => { editingDesc = true; descDraft = ''; }}>+ add description</button>
            {/if}

            <!-- Unsectioned words -->
            {#if entriesForSection(null).length > 0}
              <div class="wp-word-group">
                <div class="wp-pills">
                  {#each entriesForSection(null) as entry (entry.id)}
                    <span class="wp-pill">
                      {entry.word}
                      {#if !isSystemSelected}
                        <button class="wp-pill-x" onclick={() => handleRemoveWord(entry.id)}>&times;</button>
                      {/if}
                    </span>
                  {/each}
                </div>
              </div>
            {/if}

            <!-- Sections -->
            {#each detail.sections as sec (sec.id)}
              <div class="wp-section">
                <div class="wp-section-header">
                  {#if renamingSectionId === sec.id}
                    <input class="wp-section-rename" bind:value={renamingSectionName} onblur={handleFinishRenameSection} onkeydown={e => { if (e.key === 'Enter') handleFinishRenameSection(); }} />
                  {:else}
                    <span class="wp-section-name">{sec.name}</span>
                  {/if}
                  {#if !isSystemSelected}
                    <div class="wp-section-actions">
                      <button class="wp-icon-btn-sm" onclick={() => handleStartRenameSection(sec)} title="Rename"><i class="bi bi-pencil"></i></button>
                      <button class="wp-icon-btn-sm" onclick={() => handleDeleteSection(sec.id)} title="Delete section"><i class="bi bi-trash"></i></button>
                    </div>
                  {/if}
                </div>
                <div class="wp-pills">
                  {#each entriesForSection(sec.id) as entry (entry.id)}
                    <span class="wp-pill">
                      {entry.word}
                      {#if !isSystemSelected}
                        <button class="wp-pill-x" onclick={() => handleRemoveWord(entry.id)}>&times;</button>
                      {/if}
                    </span>
                  {/each}
                  {#if entriesForSection(sec.id).length === 0}
                    <span class="wp-empty-sec">No words yet</span>
                  {/if}
                </div>
              </div>
            {/each}

            {#if !isSystemSelected}
              <!-- Add section -->
              {#if showAddSection}
                <div class="wp-add-section-panel">
                  {#if allSectionNames.length > 0}
                    <p class="wp-add-section-label">Previously used sections:</p>
                    <div class="wp-pills wp-section-suggestions">
                      {#each allSectionNames as name}
                        <button class="wp-pill wp-pill-suggest" class:disabled={existingSectionNames().has(name)} disabled={existingSectionNames().has(name)} onclick={() => handleAddSection(name)}>{name}</button>
                      {/each}
                    </div>
                    <button class="wp-add-all-btn" onclick={handleAddAllSections}>Add all sections</button>
                  {/if}
                  <form class="wp-new-section-form" onsubmit={e => { e.preventDefault(); handleNewSection(); }}>
                    <input class="input-author" bind:value={newSectionName} placeholder="New section name..." />
                    <button class="btn-author btn-author-primary btn-author-sm" type="submit">Add</button>
                  </form>
                  <button class="wp-link-btn" onclick={() => showAddSection = false}>Close</button>
                </div>
              {:else}
                <button class="wp-link-btn" onclick={openAddSection}>+ Add Section</button>
              {/if}

              <!-- Bulk add -->
              <div class="wp-bulk">
                <div class="wp-bulk-row">
                  <label class="wp-bulk-label">Add words to:</label>
                  <select class="wp-bulk-select" bind:value={bulkSectionId}>
                    <option value="null">(No section)</option>
                    {#each detail.sections as sec}
                      <option value={sec.id}>{sec.name}</option>
                    {/each}
                  </select>
                </div>
                <textarea class="wp-bulk-input" bind:value={bulkText} rows="2" placeholder="One entry per line..."></textarea>
                <button class="btn-author btn-author-primary btn-author-sm" onclick={handleBulkAdd}>Add</button>
              </div>
            {/if}
          {:else}
            <div class="wp-detail-empty">
              <p>{isSystemSelected ? 'Select a group to view its words.' : 'Select a group to edit, or create a new one.'}</p>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <div class="wp-no-palette">
        <p>Select a palette above to manage its word groups.</p>
      </div>
    {/if}
  {/if}
</div>

<style>
  .wp-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 1rem;
  }
  .wp-title {
    font-family: var(--iwe-font-prose); font-size: 1.6rem; font-weight: 400;
    margin: 0; color: var(--iwe-text);
  }
  .wp-loading { font-size: 0.85rem; color: var(--iwe-text-muted); font-style: italic; }

  /* Palette bar */
  .wp-palette-bar {
    display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem;
    margin-bottom: 1rem; padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .wp-palette-list { display: flex; flex-wrap: wrap; gap: 0.4rem; flex: 1; }
  .wp-palette-chip {
    display: flex; align-items: center; gap: 0.2rem;
    border: 1px solid var(--iwe-border); border-radius: 20px;
    padding: 0.15rem 0.15rem 0.15rem 0.35rem;
    background: var(--iwe-bg); font-size: 0.8rem; transition: all 120ms;
  }
  .wp-palette-chip.active { border-color: var(--iwe-accent); background: var(--iwe-accent-light); }
  .wp-palette-chip.system { border-style: dashed; }
  .wp-palette-toggle {
    background: none; border: none; cursor: pointer; padding: 0; line-height: 1;
    color: var(--iwe-accent); font-size: 0.85rem;
  }
  .wp-palette-toggle .bi-circle { color: var(--iwe-text-faint); }
  .wp-palette-name {
    background: none; border: none; cursor: pointer; padding: 0.15rem 0.2rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem; font-weight: 500;
    color: var(--iwe-text);
  }
  .wp-palette-meta { font-size: 0.68rem; color: var(--iwe-text-faint); margin-left: 0.2rem; }
  .wp-system-badge { color: var(--iwe-text-faint); font-size: 0.65rem; }
  .wp-palette-action {
    background: none; border: none; cursor: pointer; padding: 0.15rem 0.25rem;
    color: var(--iwe-text-faint); font-size: 0.7rem; border-radius: 50%;
    transition: all 80ms; opacity: 0.5;
  }
  .wp-palette-chip:hover .wp-palette-action { opacity: 1; }
  .wp-palette-action:hover { color: var(--iwe-text); background: var(--iwe-bg-hover); }
  .wp-palette-delete:hover { color: var(--iwe-danger); background: var(--iwe-danger-light); }
  .wp-new-palette-btn {
    background: none; border: 1px dashed var(--iwe-border); border-radius: 20px;
    padding: 0.3rem 0.7rem; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem; color: var(--iwe-text-faint);
    transition: all 100ms;
  }
  .wp-new-palette-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .wp-new-palette-form { display: flex; gap: 0.4rem; align-items: center; }
  .wp-new-palette-form .input-author { font-size: 0.8rem; padding: 0.3rem 0.5rem; width: 160px; flex: none; }

  .wp-copy-dialog {
    background: var(--iwe-bg-warm); border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius); padding: 0.6rem 0.8rem; margin-bottom: 0.75rem;
  }
  .wp-copy-dialog form { display: flex; gap: 0.4rem; align-items: center; }
  .wp-copy-label { font-family: var(--iwe-font-ui); font-size: 0.8rem; color: var(--iwe-text-muted); white-space: nowrap; }
  .wp-copy-dialog .input-author { font-size: 0.8rem; padding: 0.3rem 0.5rem; }

  /* Panels */
  .wp-panels {
    display: flex; gap: 1px;
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius);
    background: var(--iwe-border);
    min-height: 320px; max-height: 520px;
  }
  .wp-list {
    width: 190px; flex-shrink: 0;
    background: var(--iwe-bg-sidebar);
    border-radius: var(--iwe-radius) 0 0 var(--iwe-radius);
    overflow-y: auto; padding: 0.35rem;
    display: flex; flex-direction: column;
  }
  .wp-group-item {
    display: flex; align-items: center; justify-content: space-between;
    width: 100%; background: none; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.45rem 0.6rem; text-align: left;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text); transition: background 80ms;
  }
  .wp-group-item:hover { background: var(--iwe-bg-hover); }
  .wp-group-item.active { background: var(--iwe-accent-light); color: var(--iwe-accent); }
  .wp-group-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .wp-group-count { font-size: 0.72rem; color: var(--iwe-text-faint); flex-shrink: 0; margin-left: 0.3rem; }
  .wp-empty-list { font-size: 0.8rem; color: var(--iwe-text-faint); text-align: center; padding: 1rem; font-style: italic; }
  .wp-add-group-btn {
    background: none; border: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-faint); padding: 0.4rem 0.6rem; margin-top: auto;
  }
  .wp-add-group-btn:hover { color: var(--iwe-accent); }
  .wp-new-group-form { padding: 0.4rem; margin-top: auto; }
  .wp-new-group-input { font-size: 0.8rem; padding: 0.3rem 0.5rem; width: 100%; margin-bottom: 0.3rem; }
  .wp-new-group-actions { display: flex; gap: 0.3rem; }

  .wp-detail {
    flex: 1; background: var(--iwe-bg);
    border-radius: 0 var(--iwe-radius) var(--iwe-radius) 0;
    overflow-y: auto; padding: 1rem 1.2rem;
  }
  .wp-detail-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 0.5rem;
  }
  .wp-detail-name {
    font-family: var(--iwe-font-prose); font-size: 1.1rem; font-weight: 600;
    margin: 0; color: var(--iwe-text);
  }
  .wp-detail-name-editable {
    outline: none; border-bottom: 1px dashed transparent; padding-bottom: 1px;
  }
  .wp-detail-name-editable:focus { border-bottom-color: var(--iwe-accent); }

  .wp-icon-btn {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.9rem; padding: 0.3rem;
    border-radius: var(--iwe-radius-sm); transition: all 100ms;
  }
  .wp-icon-btn:hover { color: var(--iwe-danger); background: var(--iwe-danger-light); }
  .wp-icon-btn-sm {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.72rem; padding: 0.15rem 0.25rem;
    border-radius: 3px; transition: all 100ms; opacity: 0;
  }
  .wp-section-header:hover .wp-icon-btn-sm { opacity: 1; }
  .wp-icon-btn-sm:hover { color: var(--iwe-text); background: var(--iwe-bg-hover); }

  .wp-desc {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); margin: 0 0 0.75rem;
    font-style: italic; line-height: 1.5;
  }
  .wp-desc-clickable { cursor: pointer; }
  .wp-desc-clickable:hover { color: var(--iwe-text-secondary); }
  .wp-desc-add {
    background: none; border: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-faint); padding: 0; margin-bottom: 0.75rem;
  }
  .wp-desc-add:hover { color: var(--iwe-accent); }
  .wp-desc-edit { margin-bottom: 0.75rem; }
  .wp-desc-textarea {
    width: 100%; font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    padding: 0.4rem 0.6rem; resize: vertical; outline: none;
    color: var(--iwe-text); background: var(--iwe-bg);
  }
  .wp-desc-textarea:focus { border-color: var(--iwe-accent); }
  .wp-desc-actions { display: flex; gap: 0.4rem; margin-top: 0.3rem; }

  .wp-section { margin-bottom: 0.75rem; }
  .wp-section-header {
    display: flex; align-items: center; justify-content: space-between;
    padding-bottom: 0.3rem; margin-bottom: 0.4rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .wp-section-name {
    font-family: var(--iwe-font-ui); font-size: 0.78rem; font-weight: 600;
    color: var(--iwe-text-secondary); text-transform: uppercase; letter-spacing: 0.04em;
  }
  .wp-section-actions { display: flex; gap: 0.15rem; }
  .wp-section-rename {
    font-family: var(--iwe-font-ui); font-size: 0.78rem; font-weight: 600;
    border: 1px solid var(--iwe-accent); border-radius: 3px;
    padding: 0.1rem 0.4rem; outline: none; color: var(--iwe-text);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .wp-empty-sec { font-size: 0.75rem; color: var(--iwe-text-faint); font-style: italic; }

  .wp-pills {
    display: flex; flex-wrap: wrap;
    gap: 0; line-height: 1.9;
  }
  .wp-pill {
    font-family: var(--iwe-font-prose); font-size: 0.88rem;
    padding: 0; background: none; border: none;
    color: var(--iwe-text-secondary);
    display: inline-flex; align-items: baseline; gap: 0;
    border-radius: 3px; transition: color 100ms;
    position: relative;
  }
  .wp-pill::after {
    content: '\b7';
    color: var(--iwe-text-faint);
    margin: 0 0.45rem;
    font-weight: 300;
  }
  .wp-pill:last-child::after { content: ''; margin: 0; }
  .wp-pill:hover { color: var(--iwe-accent); }
  .wp-pill-x {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-text-faint); font-size: 0.75rem; line-height: 1;
    padding: 0 0.15rem; opacity: 0; transition: opacity 80ms;
    vertical-align: super; font-family: var(--iwe-font-ui);
  }
  .wp-pill:hover .wp-pill-x { opacity: 1; }
  .wp-pill-x:hover { color: var(--iwe-danger); }
  .wp-pill-suggest { cursor: pointer; transition: all 100ms; }
  .wp-pill-suggest:hover:not(:disabled) { color: var(--iwe-accent); }
  .wp-pill-suggest:disabled { opacity: 0.4; cursor: default; }
  .wp-word-group { margin-bottom: 0.75rem; }

  .wp-add-section-panel {
    background: var(--iwe-bg-warm); border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius); padding: 0.75rem; margin: 0.75rem 0;
  }
  .wp-add-section-label { font-family: var(--iwe-font-ui); font-size: 0.75rem; color: var(--iwe-text-muted); margin: 0 0 0.4rem; }
  .wp-section-suggestions { margin-bottom: 0.5rem; }
  .wp-add-all-btn {
    font-family: var(--iwe-font-ui); font-size: 0.72rem; font-weight: 500;
    background: none; border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    padding: 0.2rem 0.5rem; cursor: pointer; color: var(--iwe-accent);
    margin-bottom: 0.5rem; transition: all 100ms;
  }
  .wp-add-all-btn:hover { background: var(--iwe-accent-light); border-color: var(--iwe-accent); }
  .wp-new-section-form { display: flex; gap: 0.4rem; margin-bottom: 0.4rem; }
  .wp-new-section-form .input-author { font-size: 0.8rem; padding: 0.3rem 0.5rem; }

  .wp-link-btn {
    background: none; border: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-faint); padding: 0; margin: 0.5rem 0;
  }
  .wp-link-btn:hover { color: var(--iwe-accent); }

  .wp-bulk {
    margin-top: 1rem; padding-top: 0.75rem;
    border-top: 1px solid var(--iwe-border-light);
  }
  .wp-bulk-row { display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.4rem; }
  .wp-bulk-label { font-family: var(--iwe-font-ui); font-size: 0.78rem; color: var(--iwe-text-muted); }
  .wp-bulk-select {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    padding: 0.2rem 0.4rem; background: var(--iwe-bg); color: var(--iwe-text);
  }
  .wp-bulk-input {
    width: 100%; font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    padding: 0.4rem 0.6rem; resize: vertical; outline: none;
    color: var(--iwe-text); background: var(--iwe-bg); margin-bottom: 0.4rem;
  }
  .wp-bulk-input:focus { border-color: var(--iwe-accent); }

  .wp-detail-empty, .wp-no-palette {
    display: flex; align-items: center; justify-content: center;
    height: 200px; color: var(--iwe-text-faint); font-style: italic;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
  }
</style>
