<script>
  import { onMount } from 'svelte';
  import { updateFormatProfile, getFormatProfiles } from '$lib/db.js';
  import { TRIM_CATEGORIES, PLATFORMS, findSize, supportedPlatforms } from '$lib/trimSizes.js';
  import { ensureUnitLoaded, getUnit, subscribe, MM_PER_IN } from '$lib/unitPreference.js';

  let { profile, onchange } = $props();

  let unit = $state('in');
  onMount(async () => {
    unit = await ensureUnitLoaded();
    return subscribe(u => { unit = u; });
  });

  let search = $state('');
  let showCustom = $state(false);
  let customW = $state('');
  let customH = $state('');

  // Current profile dimensions
  let currentW = $derived(profile?.trim_width_in ?? 6);
  let currentH = $derived(profile?.trim_height_in ?? 9);
  let currentMatch = $derived(findSize(currentW, currentH));

  function dimDisplay(inches) {
    if (unit === 'mm') return (inches * MM_PER_IN).toFixed(0);
    return String(inches);
  }

  // Always show both units — primary in the user's preference, secondary in brackets
  function dimLabel(w, h) {
    const inStr = `${w}″ × ${h}″`;
    const mmStr = `${(w * MM_PER_IN).toFixed(0)} × ${(h * MM_PER_IN).toFixed(0)} mm`;
    if (unit === 'mm') return `${mmStr} (${inStr})`;
    return `${inStr} (${mmStr})`;
  }

  // Short version for the option rows (primary unit only, secondary smaller)
  function dimPrimary(w, h) {
    if (unit === 'mm') return `${(w * MM_PER_IN).toFixed(0)} × ${(h * MM_PER_IN).toFixed(0)} mm`;
    return `${w} × ${h}″`;
  }

  function dimSecondary(w, h) {
    if (unit === 'mm') return `${w} × ${h}″`;
    return `${(w * MM_PER_IN).toFixed(0)} × ${(h * MM_PER_IN).toFixed(0)} mm`;
  }

  // Filter sizes by search
  let filteredCategories = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return TRIM_CATEGORIES;
    return TRIM_CATEGORIES.map(cat => ({
      ...cat,
      sizes: cat.sizes.filter(s =>
        s.label.toLowerCase().includes(q) ||
        cat.label.toLowerCase().includes(q) ||
        (cat.hint && cat.hint.toLowerCase().includes(q)) ||
        PLATFORMS.some(p => s[p.id] && p.label.toLowerCase().includes(q))
      ),
    })).filter(cat => cat.sizes.length > 0);
  });

  function isSelected(size) {
    return Math.abs(size.w - currentW) < 0.05 && Math.abs(size.h - currentH) < 0.05;
  }

  async function selectSize(w, h) {
    if (!profile) return;
    await updateFormatProfile(
      profile.id, profile.name, profile.target_type,
      w, h,
      profile.margin_top_in, profile.margin_bottom_in,
      profile.margin_outside_in, profile.margin_inside_in,
      profile.font_body, profile.font_size_pt, profile.line_spacing,
    );
    onchange?.();
  }

  async function applyCustom() {
    let w, h;
    if (unit === 'mm') {
      w = Number(customW) / MM_PER_IN;
      h = Number(customH) / MM_PER_IN;
    } else {
      w = Number(customW);
      h = Number(customH);
    }
    if (!Number.isFinite(w) || !Number.isFinite(h) || w < 2 || h < 2) return;
    await selectSize(w, h);
    showCustom = false;
  }
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Trim Size</h4>

  <!-- Current -->
  <div class="current-trim">
    <div class="current-trim-preview"
      style="aspect-ratio: {currentW} / {currentH};"></div>
    <div class="current-trim-info">
      <span class="current-dim">{dimLabel(currentW, currentH)}</span>
      {#if currentMatch}
        <span class="current-name">{currentMatch.label}</span>
        <div class="current-platforms">
          {#each supportedPlatforms(currentMatch) as pid}
            {@const p = PLATFORMS.find(x => x.id === pid)}
            <span class="platform-badge" title={p.label}>{p.short}</span>
          {/each}
        </div>
      {:else}
        <span class="current-name custom-label">Custom size</span>
      {/if}
    </div>
  </div>

  <!-- Search -->
  <div class="trim-search">
    <i class="bi bi-search"></i>
    <input type="text" bind:value={search}
      placeholder="Search sizes, categories, platforms..." />
  </div>

  <!-- Size catalog -->
  <div class="trim-catalog">
    {#each filteredCategories as cat}
      <div class="trim-group">
        <div class="trim-group-header">
          <span class="trim-group-label">{cat.label}</span>
          {#if cat.hint}
            <span class="trim-group-hint">{cat.hint}</span>
          {/if}
        </div>
        {#each cat.sizes as size}
          <button class="trim-option" class:selected={isSelected(size)}
            onclick={() => selectSize(size.w, size.h)}>
            <div class="trim-option-thumb"
              style="aspect-ratio: {size.w} / {size.h};"></div>
            <div class="trim-option-info">
              <span class="trim-option-dim">{dimPrimary(size.w, size.h)}</span>
              <span class="trim-option-alt">{dimSecondary(size.w, size.h)}{size.label.includes('(') ? ` · ${size.label.match(/\((.+)\)/)?.[1] || ''}` : ''}</span>
            </div>
            <div class="trim-option-platforms">
              {#each PLATFORMS as p}
                {#if size[p.id]}
                  <span class="platform-dot" class:active={true} title={p.label}>{p.short}</span>
                {/if}
              {/each}
            </div>
          </button>
        {/each}
      </div>
    {/each}
  </div>

  <!-- Custom dimensions -->
  <button class="custom-toggle" onclick={() => showCustom = !showCustom}>
    <i class="bi bi-pencil-square"></i>
    Custom dimensions
    <i class="bi bi-chevron-{showCustom ? 'up' : 'down'}" style="margin-left: auto; font-size: 0.7rem;"></i>
  </button>
  {#if showCustom}
    <div class="custom-inputs">
      <div class="custom-row">
        <label class="custom-field">
          <span>Width</span>
          <div class="custom-input-wrap">
            <input type="number" step={unit === 'mm' ? '1' : '0.1'} min="0"
              bind:value={customW} placeholder={dimDisplay(6)} />
            <span class="custom-unit">{unit === 'mm' ? 'mm' : 'in'}</span>
          </div>
        </label>
        <span class="custom-x">×</span>
        <label class="custom-field">
          <span>Height</span>
          <div class="custom-input-wrap">
            <input type="number" step={unit === 'mm' ? '1' : '0.1'} min="0"
              bind:value={customH} placeholder={dimDisplay(9)} />
            <span class="custom-unit">{unit === 'mm' ? 'mm' : 'in'}</span>
          </div>
        </label>
      </div>
      <button class="custom-apply" onclick={applyCustom}
        disabled={!customW || !customH}>
        Apply custom size
      </button>
      <p class="custom-note">
        IngramSpark accepts custom sizes from 4×6 to 8.5×11.
        Other platforms may not distribute non-standard sizes.
      </p>
    </div>
  {/if}
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }

  /* Current trim display */
  .current-trim {
    display: flex; align-items: center; gap: 0.8rem;
    padding: 0.7rem 0.8rem;
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    margin-bottom: 0.8rem;
  }
  .current-trim-preview {
    width: 36px;
    background: #fff;
    border: 1px solid var(--iwe-border);
    border-radius: 2px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }
  .current-trim-info {
    display: flex; flex-direction: column; gap: 0.15rem;
  }
  .current-dim {
    font-family: var(--iwe-font-ui); font-size: 0.95rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .current-name {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
  }
  .custom-label { font-style: italic; }
  .current-platforms {
    display: flex; gap: 3px; margin-top: 2px;
  }
  .platform-badge {
    font-family: var(--iwe-font-ui); font-size: 0.58rem;
    padding: 1px 5px; border-radius: 6px;
    background: var(--iwe-accent); color: #fff;
    font-weight: 600; letter-spacing: 0.02em;
  }

  /* Search */
  .trim-search {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    margin-bottom: 0.6rem;
  }
  .trim-search i { color: var(--iwe-text-muted); font-size: 0.8rem; }
  .trim-search input {
    flex: 1; border: none; background: none;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); outline: none;
  }

  /* Catalog */
  .trim-catalog {
    max-height: 420px; overflow-y: auto;
    margin-bottom: 0.6rem;
  }
  .trim-group { margin-bottom: 0.5rem; }
  .trim-group-header {
    display: flex; flex-direction: column; gap: 0.1rem;
    padding: 0.4rem 0 0.25rem 0;
  }
  .trim-group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .trim-group-hint {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); font-style: italic;
  }

  .trim-option {
    width: 100%;
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.45rem 0.5rem;
    border: 1px solid transparent; border-radius: var(--iwe-radius-sm);
    background: none; cursor: pointer;
    font-family: var(--iwe-font-ui); text-align: left;
    transition: all 100ms;
  }
  .trim-option:hover { background: var(--iwe-bg-hover); border-color: var(--iwe-border); }
  .trim-option.selected {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
  }
  .trim-option-thumb {
    width: 22px; flex-shrink: 0;
    background: #fff;
    border: 1px solid var(--iwe-border);
    border-radius: 1px;
  }
  .trim-option.selected .trim-option-thumb { border-color: var(--iwe-accent); }
  .trim-option-info {
    flex: 1; min-width: 0;
    display: flex; flex-direction: column;
  }
  .trim-option-dim {
    font-size: 0.82rem; color: var(--iwe-text);
  }
  .trim-option.selected .trim-option-dim { color: var(--iwe-accent); font-weight: 500; }
  .trim-option-alt {
    font-size: 0.65rem; color: var(--iwe-text-muted);
  }
  .trim-option-platforms {
    display: flex; flex-wrap: wrap; gap: 2px;
    flex-shrink: 0;
  }
  .platform-dot {
    font-size: 0.5rem;
    padding: 1px 4px; border-radius: 4px;
    background: var(--iwe-bg-hover); color: var(--iwe-text-muted);
    font-weight: 600; letter-spacing: 0.02em;
  }
  .platform-dot.active {
    background: rgba(45, 106, 94, 0.12); color: var(--iwe-accent);
  }

  /* Custom dimensions */
  .custom-toggle {
    width: 100%;
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.55rem 0.6rem;
    border: 1px dashed var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: none; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted);
    transition: all 120ms;
  }
  .custom-toggle:hover {
    color: var(--iwe-accent); border-color: var(--iwe-accent);
  }
  .custom-inputs {
    padding: 0.7rem;
    border: 1px solid var(--iwe-border); border-top: none;
    border-radius: 0 0 var(--iwe-radius-sm) var(--iwe-radius-sm);
    background: var(--iwe-bg-warm);
  }
  .custom-row {
    display: flex; align-items: flex-end; gap: 0.4rem;
    margin-bottom: 0.6rem;
  }
  .custom-x {
    font-size: 1rem; color: var(--iwe-text-muted);
    padding-bottom: 0.35rem;
  }
  .custom-field {
    flex: 1; display: flex; flex-direction: column; gap: 0.2rem;
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    font-weight: 600; letter-spacing: 0.03em;
  }
  .custom-input-wrap {
    display: flex; align-items: center;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); overflow: hidden;
  }
  .custom-input-wrap:focus-within { border-color: var(--iwe-accent); }
  .custom-input-wrap input {
    flex: 1; min-width: 0; border: none; background: none;
    padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); outline: none;
  }
  .custom-input-wrap input::-webkit-outer-spin-button,
  .custom-input-wrap input::-webkit-inner-spin-button {
    -webkit-appearance: none; margin: 0;
  }
  .custom-unit {
    padding: 0 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border-left: 1px solid var(--iwe-border);
    height: 100%; display: flex; align-items: center;
  }
  .custom-apply {
    width: 100%;
    padding: 0.45rem 0.8rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    background: var(--iwe-accent); border: 1px solid var(--iwe-accent);
    color: #fff; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 100ms;
  }
  .custom-apply:hover:not(:disabled) { background: #245a4f; }
  .custom-apply:disabled { opacity: 0.4; cursor: not-allowed; }
  .custom-note {
    margin: 0.5rem 0 0;
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); line-height: 1.4;
  }
</style>
