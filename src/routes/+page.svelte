<script>
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { open } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { getProjectsDir, setProjectsDir, listProjects, createProject, deleteProject, getSettings, saveSettings, setSpellLanguage } from '$lib/db.js';
  import WordPalettes from '$lib/components/WordPalettes.svelte';

  let projectsDir = $state(null);
  let projects = $state([]);
  let newTitle = $state('');
  let showNewForm = $state(false);
  let loading = $state(true);

  // Settings
  let spellLang = $state('en_US');

  // Navigation
  let activeView = $state('projects');

  onMount(async () => {
    projectsDir = await getProjectsDir();
    if (projectsDir) {
      projects = await listProjects();
    }
    const settings = await getSettings();
    if (settings.spellLanguage) {
      spellLang = settings.spellLanguage;
      try { await setSpellLanguage(spellLang); } catch {}
    }
    loading = false;
  });

  async function handleSpellLangChange(e) {
    spellLang = e.target.value;
    const settings = await getSettings();
    settings.spellLanguage = spellLang;
    await saveSettings(settings);
    try { await setSpellLanguage(spellLang); } catch {}
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

  async function handleDelete(e, project) {
    e.stopPropagation();
    if (confirm(`Delete "${project.title}"? This cannot be undone.`)) {
      await deleteProject(project.filepath);
      projects = await listProjects();
    }
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
            <h3 class="settings-heading">Storage</h3>
            <div class="settings-row">
              <label class="settings-label">Projects folder</label>
              <div class="settings-path-row">
                <span class="settings-path">{projectsDir}</span>
                <button class="btn-author btn-author-subtle btn-author-sm" onclick={pickFolder}>Change</button>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </main>
  </div>
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
    font-size: 1.4rem; cursor: pointer; padding: 0.5rem 0.75rem;
    opacity: 0; transition: all 150ms; line-height: 1;
  }
  .project-delete:hover { color: var(--iwe-danger); }
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
  .settings-path-row {
    display: flex; align-items: center; gap: 0.5rem;
  }
  .settings-path {
    font-size: 0.8rem; color: var(--iwe-text-muted);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    max-width: 280px;
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
