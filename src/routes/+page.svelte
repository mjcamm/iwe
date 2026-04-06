<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { open } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { listen } from '@tauri-apps/api/event';
  import { getProjectsDir, setProjectsDir, listProjects, createProject, deleteProject, getSettings, saveSettings, setSpellLanguage } from '$lib/db.js';
  import WordPalettes from '$lib/components/WordPalettes.svelte';

  let projectsDir = $state(null);
  let projects = $state([]);
  let newTitle = $state('');
  let showNewForm = $state(false);
  let loading = $state(true);

  // Settings
  let spellLang = $state('en_US');
  let semanticIndexDelay = $state(30);
  let typewriterMode = $state(false);
  let backupInterval = $state(60);

  // Navigation
  let activeView = $state('projects');
  const isDev = import.meta.env.DEV;

  function handleAnalyseProject(e, project) {
    e.preventDefault();
    e.stopPropagation();
    const label = 'analyse-' + Date.now();
    new WebviewWindow(label, {
      url: '/analyse?filepath=' + encodeURIComponent(project.filepath),
      title: 'Analyse — ' + project.title,
      width: 1100,
      height: 800,
      resizable: true
    });
  }

  onMount(async () => {
    projectsDir = await getProjectsDir();
    if (projectsDir) {
      projects = await listProjects();
    }
    const settings = await getSettings();
    if (settings.spellLanguage) {
      spellLang = settings.spellLanguage;
    }
    if (settings.semanticIndexDelay !== undefined) {
      semanticIndexDelay = settings.semanticIndexDelay;
    }
    if (settings.typewriterMode !== undefined) {
      typewriterMode = settings.typewriterMode;
    }
    if (settings.backupInterval !== undefined) {
      backupInterval = settings.backupInterval;
      try { await setSpellLanguage(spellLang); } catch {}
    }
    loading = false;

    // Refresh project list when an import completes
    await listen('projects-changed', async () => {
      projects = await listProjects();
    });
  });

  async function handleImport() {
    const selected = await open({
      title: 'Import manuscript',
      filters: [{ name: 'Manuscript', extensions: ['docx', 'epub'] }]
    });
    if (!selected) return;
    const path = typeof selected === 'string' ? selected : selected.path;
    const label = 'import-' + Date.now();
    new WebviewWindow(label, {
      url: '/import?path=' + encodeURIComponent(path),
      title: 'Import manuscript',
      width: 1100,
      height: 800,
      resizable: true
    });
  }

  async function handleSpellLangChange(e) {
    spellLang = e.target.value;
    const settings = await getSettings();
    settings.spellLanguage = spellLang;
    await saveSettings(settings);
    try { await setSpellLanguage(spellLang); } catch {}
  }

  async function handleTypewriterToggle(e) {
    typewriterMode = e.target.checked;
    const settings = await getSettings();
    settings.typewriterMode = typewriterMode;
    await saveSettings(settings);
  }

  async function handleBackupIntervalChange(e) {
    backupInterval = parseInt(e.target.value) || 0;
    const settings = await getSettings();
    settings.backupInterval = backupInterval;
    await saveSettings(settings);
  }

  async function handleSemanticDelayChange(e) {
    semanticIndexDelay = parseInt(e.target.value) || 0;
    const settings = await getSettings();
    settings.semanticIndexDelay = semanticIndexDelay;
    await saveSettings(settings);
  }

  async function pickFolder() {
    const selected = await open({ directory: true, title: 'Choose where to store your projects' });
    if (selected) {
      projectsDir = selected;
      await setProjectsDir(selected);
      projects = await listProjects();
    }
  }

  async function handleCreate() {
    if (!newTitle.trim()) return;
    const filename = await createProject(newTitle.trim());
    newTitle = '';
    showNewForm = false;
    goto(`/project/${encodeURIComponent(filename)}`);
  }

  // Delete confirmation modal
  let deleteModal = $state({ show: false, project: null });
  let deleteConfirmText = $state('');

  function handleDelete(e, project) {
    e.stopPropagation();
    deleteConfirmText = '';
    deleteModal = { show: true, project };
  }

  async function confirmDelete() {
    if (!deleteModal.project) return;
    await deleteProject(deleteModal.project.filepath);
    deleteModal = { show: false, project: null };
    deleteConfirmText = '';
    projects = await listProjects();
  }

  function cancelDelete() {
    deleteModal = { show: false, project: null };
    deleteConfirmText = '';
  }

  async function openFolder() {
    if (projectsDir) {
      await revealItemInDir(projectsDir);
    }
  }
</script>

{#if loading}
  <div class="home-loading">
    <div class="loading-dot"></div>
  </div>
{:else if !projectsDir}
  <div class="home-setup">
    <div class="setup-inner">
      <h1 class="setup-title">IWE</h1>
      <div class="setup-rule"></div>
      <p class="setup-subtitle">Integrated Writing Environment</p>
      <p class="setup-desc">Choose a folder to store your manuscripts.<br/>Your Documents folder works nicely.</p>
      <button class="btn-author btn-author-primary btn-author-lg" onclick={pickFolder}>Choose Folder</button>
    </div>
  </div>
{:else}
  <div class="home-shell">
    <nav class="home-sidebar">
      <div class="sidebar-brand">
        <span class="brand-title">IWE</span>
      </div>
      <div class="sidebar-nav">
        <button class="nav-item" class:active={activeView === 'projects'} onclick={() => activeView = 'projects'}>
          <i class="bi bi-journal-bookmark"></i>
          <span>Projects</span>
        </button>
        <button class="nav-item" class:active={activeView === 'palettes'} onclick={() => activeView = 'palettes'}>
          <i class="bi bi-palette2"></i>
          <span>Word Palettes</span>
        </button>
        <button class="nav-item" class:active={activeView === 'settings'} onclick={() => activeView = 'settings'}>
          <i class="bi bi-gear"></i>
          <span>Settings</span>
        </button>
      </div>
    </nav>

    <main class="home-content">
      {#if activeView === 'projects'}
        <div class="view-inner">
          <header class="view-header">
            <h1 class="view-title">Manuscripts</h1>
          </header>

          <div class="home-actions">
            {#if showNewForm}
              <form class="new-form" onsubmit={e => { e.preventDefault(); handleCreate(); }}>
                <input class="input-author" bind:value={newTitle} placeholder="Untitled manuscript..." />
                <button class="btn-author btn-author-primary" type="submit">Create</button>
                <button class="btn-author btn-author-subtle" type="button" onclick={() => { showNewForm = false; newTitle = ''; }}>Cancel</button>
              </form>
            {:else}
              <div class="action-row">
                <button class="btn-author btn-author-primary" onclick={() => showNewForm = true}>+ New Manuscript</button>
                <button class="btn-author btn-author-subtle" onclick={handleImport}>Import…</button>
                <div class="action-links">
                  <button class="btn-text" onclick={async () => { projects = await listProjects(); }}>Refresh</button>
                  <span class="dot-sep"></span>
                  <button class="btn-text" onclick={openFolder}>Open Folder</button>
                  <span class="dot-sep"></span>
                  <button class="btn-text" onclick={pickFolder}>Change Folder</button>
                </div>
              </div>
            {/if}
          </div>

          <div class="home-path"><span>{projectsDir}</span></div>

          {#if projects.length > 0}
            <ul class="project-list">
              {#each projects as project (project.filepath)}
                <li class="project-item">
                  <a href="/project/{encodeURIComponent(project.filename)}" class="project-link">
                    <span class="project-icon">&#9782;</span>
                    <div class="project-meta">
                      <span class="project-name">{project.title}</span>
                      <span class="project-file">{project.filename}</span>
                    </div>
                  </a>
                  {#if isDev}
                    <button class="project-analyse" onclick={e => handleAnalyseProject(e, project)} title="Analyse (dev only)">⚗</button>
                  {/if}
                  <button class="project-delete" onclick={e => handleDelete(e, project)} title="Delete">&times;</button>
                </li>
              {/each}
            </ul>
          {:else}
            <div class="home-empty">
              <p>No manuscripts yet.</p>
              <p class="hint">Create one above, or drop a <code>.iwe</code> file into your projects folder.</p>
            </div>
          {/if}
        </div>

      {:else if activeView === 'palettes'}
        <div class="view-inner view-inner-wide">
          <WordPalettes />
        </div>

      {:else if activeView === 'settings'}
        <div class="view-inner">
          <header class="view-header">
            <h1 class="view-title">Settings</h1>
          </header>

          <div class="settings-section">
            <h3 class="settings-heading">Spell Check</h3>
            <div class="settings-row">
              <label class="settings-label">Dictionary language</label>
              <select class="settings-select" value={spellLang} onchange={handleSpellLangChange}>
                <option value="en_US">English (US)</option>
                <option value="en_GB">English (UK)</option>
              </select>
            </div>
          </div>

          <div class="settings-section">
            <h3 class="settings-heading">Editor</h3>
            <div class="settings-row">
              <label class="settings-label">Typewriter mode</label>
              <input class="settings-checkbox" type="checkbox" checked={typewriterMode} onchange={handleTypewriterToggle} />
            </div>
            <p class="settings-hint">Keeps the cursor vertically centered on screen as you type, like a typewriter. The page scrolls to keep your writing position in the middle of the editor.</p>
          </div>

          <div class="settings-section">
            <h3 class="settings-heading">Descriptive Search</h3>
            <div class="settings-row">
              <label class="settings-label">Indexing delay (seconds)</label>
              <input class="settings-input" type="number" min="0" max="300" step="5" value={semanticIndexDelay} onchange={handleSemanticDelayChange} />
            </div>
            <p class="settings-hint">How many seconds of inactivity before re-indexing a chapter for meaning search. Set to 0 to disable automatic indexing (you can still rebuild manually).</p>
          </div>

          <div class="settings-section">
            <h3 class="settings-heading">Storage</h3>
            <div class="settings-row">
              <label class="settings-label">Projects folder</label>
              <div class="settings-path-row">
                <span class="settings-path">{projectsDir}</span>
                <button class="btn-author btn-author-subtle btn-author-sm" onclick={pickFolder}>Change</button>
              </div>
            </div>
          </div>

          <div class="settings-section">
            <h3 class="settings-heading">Backups</h3>
            <div class="settings-row">
              <label class="settings-label">Backup interval (minutes)</label>
              <input class="settings-input" type="number" min="0" max="1440" step="5" value={backupInterval} onchange={handleBackupIntervalChange} />
            </div>
            <p class="settings-hint">How often to automatically back up the project while writing. Set to 0 to disable automatic backups. Backups are stored in a "backups" folder next to your project file, with semantic index data stripped to save space.</p>
            {#if projectsDir}
              <p class="settings-hint">
                Backups location: <button class="settings-link" onclick={() => { import('@tauri-apps/plugin-opener').then(m => m.openPath(projectsDir + '/backups')).catch(() => {}); }}>{projectsDir}/backups</button>
              </p>
            {/if}
            <p class="settings-hint">Backups within 7 days are kept in full. Older backups are pruned to 1 per day.</p>
          </div>

          <div class="settings-section settings-credits">
            <h3 class="settings-heading">Licenses</h3>
            <p class="settings-hint">Descriptive Search powered by all-mpnet-base-v2 (Apache 2.0) — sentence-transformers/all-mpnet-base-v2</p>
            <p class="settings-hint">ONNX Runtime (MIT License) — microsoft/onnxruntime</p>
          </div>
        </div>
      {/if}
    </main>
  </div>

  {#if deleteModal.show}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="del-backdrop" onclick={cancelDelete}>
      <div class="del-modal" onclick={(e) => e.stopPropagation()}>
        <div class="del-header">
          <span class="del-title">Delete project</span>
          <button class="del-close" onclick={cancelDelete}>&times;</button>
        </div>
        <div class="del-body">
          <p class="del-msg">
            This will permanently delete <strong>"{deleteModal.project.title}"</strong> and all its contents. This cannot be undone.
          </p>
          <label class="del-label">
            Type <strong>delete</strong> to confirm:
          </label>
          <input
            class="del-input"
            type="text"
            bind:value={deleteConfirmText}
            placeholder="delete"
            onkeydown={(e) => { if (e.key === 'Enter' && deleteConfirmText.toLowerCase() === 'delete') confirmDelete(); }}
          />
        </div>
        <div class="del-footer">
          <button class="btn-author btn-author-subtle" onclick={cancelDelete}>Cancel</button>
          <button
            class="del-confirm-btn"
            disabled={deleteConfirmText.toLowerCase() !== 'delete'}
            onclick={confirmDelete}
          >Delete project</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .home-loading {
    display: flex; align-items: center; justify-content: center; height: 100vh;
  }
  .loading-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: var(--iwe-text-muted);
    animation: blink 1s ease infinite;
  }
  @keyframes blink { 0%,100% { opacity: 0.2; } 50% { opacity: 1; } }

  .home-setup {
    display: flex; align-items: center; justify-content: center;
    height: 100vh; background: var(--iwe-bg-warm);
  }
  .setup-inner { text-align: center; max-width: 400px; padding: 2rem; }
  .setup-title {
    font-family: var(--iwe-font-prose); font-size: 2.8rem; font-weight: 400;
    letter-spacing: 0.15em; margin: 0 0 1rem; color: var(--iwe-text);
  }
  .setup-rule {
    width: 50px; height: 1px; background: var(--iwe-accent);
    margin: 0 auto 1rem; opacity: 0.6;
  }
  .setup-subtitle {
    font-size: 0.85rem; color: var(--iwe-text-muted);
    letter-spacing: 0.08em; text-transform: uppercase; margin: 0 0 1.5rem;
  }
  .setup-desc {
    font-family: var(--iwe-font-prose); font-size: 0.95rem;
    color: var(--iwe-text-secondary); line-height: 1.7; margin-bottom: 2rem;
  }

  /* Shell layout */
  .home-shell {
    display: flex; height: 100vh; background: var(--iwe-bg-warm);
  }

  /* Sidebar */
  .home-sidebar {
    width: 190px; flex-shrink: 0;
    background: var(--iwe-bg-sidebar);
    border-right: 1px solid var(--iwe-border);
    display: flex; flex-direction: column;
  }
  .sidebar-brand {
    padding: 1.4rem 1.2rem 1rem;
  }
  .brand-title {
    font-family: var(--iwe-font-prose); font-size: 1.3rem; font-weight: 400;
    letter-spacing: 0.12em; color: var(--iwe-text);
  }
  .sidebar-nav {
    display: flex; flex-direction: column; gap: 2px;
    padding: 0 0.5rem;
  }
  .nav-item {
    display: flex; align-items: center; gap: 0.6rem;
    width: 100%; background: none; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.55rem 0.7rem; text-align: left;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text-secondary); transition: all 100ms;
  }
  .nav-item:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .nav-item.active {
    background: var(--iwe-accent-light); color: var(--iwe-accent); font-weight: 500;
  }
  .nav-item i { font-size: 0.95rem; width: 1.1rem; text-align: center; }

  /* Content area */
  .home-content {
    flex: 1; overflow-y: auto;
  }
  .view-inner { max-width: 560px; margin: 0 auto; padding: 2.5rem 2rem 4rem; }
  .view-inner-wide { max-width: 780px; }
  .view-header { margin-bottom: 1.5rem; }
  .view-title {
    font-family: var(--iwe-font-prose); font-size: 1.6rem; font-weight: 400;
    margin: 0; color: var(--iwe-text);
  }

  /* Projects view */
  .home-actions { margin-bottom: 0.75rem; }
  .action-row { display: flex; align-items: center; justify-content: space-between; }
  .action-links { display: flex; align-items: center; gap: 0.5rem; }
  .dot-sep { width: 3px; height: 3px; border-radius: 50%; background: var(--iwe-text-faint); }
  .new-form { display: flex; gap: 0.5rem; }

  .home-path {
    font-size: 0.75rem; color: var(--iwe-text-faint); margin-bottom: 1.5rem;
    padding-bottom: 1rem; border-bottom: 1px solid var(--iwe-border-light);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  .project-list { list-style: none; padding: 0; margin: 0; }
  .project-item {
    display: flex; align-items: center;
    border-bottom: 1px solid var(--iwe-border-light);
    transition: background 150ms ease;
  }
  .project-item:hover { background: var(--iwe-bg-hover); }
  .project-item:hover .project-delete { opacity: 1; }
  .project-link {
    flex: 1; display: flex; align-items: center; gap: 0.75rem;
    padding: 0.85rem 0.5rem; text-decoration: none; color: inherit; min-width: 0;
  }
  .project-icon { font-size: 1.2rem; color: var(--iwe-accent); opacity: 0.5; flex-shrink: 0; }
  .project-meta { display: flex; flex-direction: column; min-width: 0; }
  .project-name {
    font-family: var(--iwe-font-prose); font-size: 0.95rem; color: var(--iwe-text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .project-file { font-size: 0.7rem; color: var(--iwe-text-faint); }
  .project-delete {
    background: none; border: none; color: var(--iwe-text-faint);
    font-size: 2.6rem; cursor: pointer; padding: 0.7rem 1.1rem;
    opacity: 0; transition: all 150ms; line-height: 1;
  }
  .project-delete:hover { color: var(--iwe-danger); }
  .project-analyse {
    background: none; border: none; color: var(--iwe-text-faint);
    font-size: 2rem; cursor: pointer; padding: 0.7rem 0.9rem;
    opacity: 0; transition: all 150ms; line-height: 1;
  }
  .project-item:hover .project-analyse { opacity: 1; }
  .project-analyse:hover { color: var(--iwe-accent); }
  .home-empty {
    text-align: center; padding: 3rem 1rem;
    font-family: var(--iwe-font-prose); color: var(--iwe-text-muted);
  }
  .hint { font-size: 0.85rem; color: var(--iwe-text-faint); margin-top: 0.25rem; }
  .hint code {
    font-size: 0.8em; background: var(--iwe-bg-hover);
    padding: 0.1rem 0.35rem; border-radius: 3px;
  }

  /* Settings view */
  .settings-section {
    margin-bottom: 2rem;
  }
  .settings-heading {
    font-family: var(--iwe-font-ui); font-size: 0.8rem; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--iwe-text-secondary); margin: 0 0 0.75rem;
    padding-bottom: 0.5rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .settings-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.5rem 0;
  }
  .settings-label {
    font-family: var(--iwe-font-ui); font-size: 0.88rem; color: var(--iwe-text);
  }
  .settings-select {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); padding: 0.35rem 0.6rem;
    color: var(--iwe-text); cursor: pointer;
  }
  .settings-select:focus { border-color: var(--iwe-accent); outline: none; }
  .settings-input {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); padding: 0.35rem 0.6rem;
    color: var(--iwe-text); width: 80px; text-align: center;
  }
  .settings-input:focus { border-color: var(--iwe-accent); outline: none; }
  .settings-link {
    background: none; border: none; cursor: pointer;
    color: var(--iwe-accent); font-family: var(--iwe-font-ui);
    font-size: 0.75rem; font-style: italic; padding: 0;
    text-decoration: underline; text-underline-offset: 2px;
  }
  .settings-link:hover { opacity: 0.8; }
  .settings-checkbox {
    width: 18px; height: 18px; accent-color: var(--iwe-accent);
    cursor: pointer;
  }
  .settings-hint {
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-faint); font-style: italic;
    margin: 0.3rem 0 0; line-height: 1.4;
  }
  .settings-path-row {
    display: flex; align-items: center; gap: 0.5rem;
  }
  .settings-path {
    font-size: 0.8rem; color: var(--iwe-text-muted);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    max-width: 280px;
  }

  /* Delete confirmation modal */
  .del-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.4);
    display: flex; align-items: center; justify-content: center;
    animation: del-fade 0.15s ease;
  }
  @keyframes del-fade { from { opacity: 0; } to { opacity: 1; } }
  .del-modal {
    background: var(--iwe-bg); border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2);
    width: 90vw; max-width: 420px;
    animation: del-slide 0.2s ease;
  }
  @keyframes del-slide {
    from { opacity: 0; transform: translateY(10px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .del-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1rem 1.2rem; border-bottom: 1px solid var(--iwe-border);
  }
  .del-title {
    font-family: var(--iwe-font-ui); font-size: 1rem; font-weight: 600;
    color: var(--iwe-danger);
  }
  .del-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.5rem; line-height: 1; color: var(--iwe-text-faint);
    padding: 0.2rem 0.4rem; border-radius: var(--iwe-radius-sm);
  }
  .del-close:hover { color: var(--iwe-text); background: var(--iwe-bg-hover); }
  .del-body { padding: 1.2rem; }
  .del-msg {
    font-family: var(--iwe-font-ui); font-size: 0.88rem;
    color: var(--iwe-text); line-height: 1.6; margin: 0 0 1rem;
  }
  .del-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-secondary); display: block; margin-bottom: 0.4rem;
  }
  .del-input {
    width: 100%; font-family: var(--iwe-font-ui); font-size: 0.9rem;
    padding: 0.5rem 0.7rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); outline: none;
    color: var(--iwe-text); background: var(--iwe-bg);
  }
  .del-input:focus { border-color: var(--iwe-danger); }
  .del-footer {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    padding: 0.8rem 1.2rem; border-top: 1px solid var(--iwe-border-light);
  }
  .del-confirm-btn {
    font-family: var(--iwe-font-ui); font-size: 0.85rem; font-weight: 500;
    padding: 0.45rem 1rem; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; background: var(--iwe-danger); color: white;
    transition: all 150ms;
  }
  .del-confirm-btn:disabled {
    opacity: 0.35; cursor: not-allowed;
  }
  .del-confirm-btn:not(:disabled):hover {
    background: var(--iwe-danger-hover, #a0403d);
  }

  /* Shared components */
  :global(.btn-author) {
    font-family: var(--iwe-font-ui); font-size: 0.85rem; font-weight: 500;
    padding: 0.45rem 1rem; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 150ms;
    display: inline-flex; align-items: center; gap: 0.3rem; white-space: nowrap;
  }
  :global(.btn-author-primary) { background: var(--iwe-accent); color: white; }
  :global(.btn-author-primary:hover) { background: var(--iwe-accent-hover); }
  :global(.btn-author-subtle) { background: var(--iwe-bg-hover); color: var(--iwe-text-secondary); }
  :global(.btn-author-subtle:hover) { background: var(--iwe-bg-active); }
  :global(.btn-author-lg) { padding: 0.6rem 1.5rem; font-size: 0.9rem; }
  :global(.btn-author-sm) { padding: 0.3rem 0.7rem; font-size: 0.78rem; }

  .btn-text {
    background: none; border: none; font-family: var(--iwe-font-ui);
    font-size: 0.8rem; color: var(--iwe-text-muted); cursor: pointer;
    padding: 0.2rem 0;
  }
  .btn-text:hover { color: var(--iwe-accent); }

  :global(.input-author) {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    padding: 0.45rem 0.7rem; flex: 1; outline: none;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); color: var(--iwe-text);
    transition: border-color 150ms;
  }
  :global(.input-author:focus) { border-color: var(--iwe-accent); }
  :global(.input-author::placeholder) { color: var(--iwe-text-faint); }
</style>
