<script>
  import { onMount } from 'svelte';
  import { ensureFontList } from '$lib/fontList.js';

  // Reusable font picker — used in Typography, chapter headings, headers/footers,
  // page numbers, page editor, etc. Self-contained: fetches the system font list once
  // (cached) and renders each option in its own face.
  let { value = $bindable(), placeholder = 'Pick a font...', onchange } = $props();

  let allFonts = $state([]);
  let loading = $state(true);
  let open = $state(false);
  let search = $state('');
  let inputEl;

  onMount(async () => {
    allFonts = await ensureFontList();
    loading = false;
  });

  let filtered = $derived(() => {
    const q = search.trim().toLowerCase();
    if (!q) return allFonts;
    return allFonts.filter(f => f.toLowerCase().includes(q));
  });

  function selectFont(font) {
    value = font;
    open = false;
    search = '';
    onchange?.(font);
  }

  function toggleOpen() {
    open = !open;
    if (open) {
      // Focus the search box when opening
      setTimeout(() => inputEl?.focus(), 0);
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      open = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="font-picker">
  <button class="font-picker-trigger" onclick={toggleOpen} type="button">
    <span class="font-picker-current" style:font-family={value || 'inherit'}>
      {value || placeholder}
    </span>
    <i class="bi bi-chevron-down picker-chevron" class:open></i>
  </button>

  {#if open}
    <div class="font-picker-backdrop"
      onclick={() => open = false}
      role="button" tabindex="-1" onkeydown={() => {}}></div>
    <div class="font-picker-dropdown">
      <div class="font-picker-search">
        <i class="bi bi-search"></i>
        <input
          bind:this={inputEl}
          bind:value={search}
          placeholder="Search fonts..."
          type="text"
        />
      </div>
      <div class="font-picker-list">
        {#if loading}
          <div class="font-picker-empty">Loading...</div>
        {:else if filtered().length === 0}
          <div class="font-picker-empty">No fonts match "{search}"</div>
        {:else}
          {#each filtered() as font (font)}
            <button class="font-picker-option"
              class:active={font === value}
              style:font-family={font}
              onclick={() => selectFont(font)}>
              {font}
              {#if font === value}
                <i class="bi bi-check2"></i>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .font-picker {
    position: relative;
    width: 100%;
  }
  .font-picker-trigger {
    width: 100%;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
    text-align: left;
  }
  .font-picker-trigger:hover { border-color: var(--iwe-accent); }
  .font-picker-current {
    flex: 1;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-size: 0.95rem;
  }
  .picker-chevron {
    color: var(--iwe-text-muted); font-size: 0.7rem;
    transition: transform 150ms;
  }
  .picker-chevron.open { transform: rotate(180deg); }

  .font-picker-backdrop {
    position: fixed; inset: 0; z-index: 50;
    background: transparent; cursor: default;
  }
  .font-picker-dropdown {
    position: absolute; top: calc(100% + 4px); left: 0; right: 0;
    z-index: 100;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    box-shadow: 0 8px 32px rgba(0,0,0,0.18);
    display: flex; flex-direction: column;
    max-height: 380px;
    overflow: hidden;
  }
  .font-picker-search {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.7rem;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
  }
  .font-picker-search i {
    color: var(--iwe-text-muted); font-size: 0.8rem;
  }
  .font-picker-search input {
    flex: 1;
    border: none; background: none;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text);
    outline: none;
  }
  .font-picker-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }
  .font-picker-option {
    width: 100%;
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.45rem 0.6rem;
    border: none; background: none; color: var(--iwe-text); cursor: pointer;
    border-radius: var(--iwe-radius-sm);
    font-size: 1rem;
    text-align: left;
    line-height: 1.2;
    transition: background 80ms;
  }
  .font-picker-option:hover { background: var(--iwe-bg-hover); }
  .font-picker-option.active {
    background: rgba(45, 106, 94, 0.08);
    color: var(--iwe-accent);
  }
  .font-picker-option i {
    color: var(--iwe-accent); font-size: 0.9rem;
    flex-shrink: 0;
  }
  .font-picker-empty {
    padding: 1rem;
    text-align: center;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); font-style: italic;
  }
</style>
