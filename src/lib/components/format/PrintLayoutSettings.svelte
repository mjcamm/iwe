<script>
  import { onMount } from 'svelte';
  import { updateProfileCategory } from '$lib/db.js';
  import { getRecommendedMargins } from '$lib/marginDefaults.js';
  import { ensureUnitLoaded, getUnit, toDisplay, fromDisplay, unitLabel, unitStep, subscribe } from '$lib/unitPreference.js';

  let { profile, onchange } = $props();

  let unit = $state('in');
  let unitLoaded = $state(false);

  onMount(async () => {
    unit = await ensureUnitLoaded();
    unitLoaded = true;
    return subscribe(u => { unit = u; });
  });

  // Canonical storage is inches. Read from print_layout_json with fallback to scalar columns.
  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.print_layout_json || '{}');
      return {
        margin_top_in:     parsed.margin_top_in     ?? profile?.margin_top_in     ?? 0.875,
        margin_bottom_in:  parsed.margin_bottom_in  ?? profile?.margin_bottom_in  ?? 0.875,
        margin_outside_in: parsed.margin_outside_in ?? profile?.margin_outside_in ?? 0.625,
        margin_inside_in:  parsed.margin_inside_in  ?? profile?.margin_inside_in  ?? 0.875,
        justify: parsed.justify ?? true,
        hyphens: parsed.hyphens ?? true,
        bleed_enabled: parsed.bleed_enabled ?? false,
        bleed_in: parsed.bleed_in ?? 0.125,
      };
    } catch {
      return { margin_top_in: 0.875, margin_bottom_in: 0.875, margin_outside_in: 0.625, margin_inside_in: 0.875, justify: true, hyphens: true, bleed_enabled: false, bleed_in: 0.125 };
    }
  });

  // Local working copies (inches, canonical)
  let topIn    = $state(settings.margin_top_in);
  let bottomIn = $state(settings.margin_bottom_in);
  let outsideIn = $state(settings.margin_outside_in);
  let insideIn = $state(settings.margin_inside_in);
  let justify  = $state(settings.justify);
  let hyphens  = $state(settings.hyphens);
  let bleedEnabled = $state(settings.bleed_enabled);
  let bleedIn = $state(settings.bleed_in);

  // Re-sync when profile prop changes
  $effect(() => {
    topIn     = settings.margin_top_in;
    bottomIn  = settings.margin_bottom_in;
    outsideIn = settings.margin_outside_in;
    insideIn  = settings.margin_inside_in;
    justify   = settings.justify;
    hyphens   = settings.hyphens;
    bleedEnabled = settings.bleed_enabled;
    bleedIn  = settings.bleed_in;
  });

  let uLabel = $derived(unitLabel());
  let step = $derived(unitStep());

  // ---- Save (debounced) ----
  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 300);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      margin_top_in:     topIn,
      margin_bottom_in:  bottomIn,
      margin_outside_in: outsideIn,
      margin_inside_in:  insideIn,
      justify,
      hyphens,
      bleed_enabled: bleedEnabled,
      bleed_in: bleedIn,
    });
    await updateProfileCategory(profile.id, 'print_layout_json', json);
    onchange?.();
  }

  function toggleJustify() {
    justify = !justify;
    scheduleSave();
  }

  function toggleHyphens() {
    hyphens = !hyphens;
    scheduleSave();
  }

  function toggleBleed() {
    bleedEnabled = !bleedEnabled;
    scheduleSave();
  }

  function handleBleedInput(e) {
    const inches = fromDisplay(e.target.value);
    if (inches == null) return;
    bleedIn = inches;
    scheduleSave();
  }

  function handleInput(field, e) {
    const inches = fromDisplay(e.target.value);
    if (inches == null) return;
    if (field === 'top')     topIn = inches;
    if (field === 'bottom')  bottomIn = inches;
    if (field === 'outside') outsideIn = inches;
    if (field === 'inside')  insideIn = inches;
    scheduleSave();
  }

  async function resetToRecommended() {
    if (!profile) return;
    const m = getRecommendedMargins(profile.trim_width_in, profile.trim_height_in);
    topIn = m.top;
    bottomIn = m.bottom;
    outsideIn = m.outside;
    insideIn = m.inside;
    // Force an immediate save — don't wait for the debounce
    clearTimeout(saveTimer);
    await persist();
  }

  // Label showing the current trim in the user's unit
  const trimLabel = $derived.by(() => {
    if (!profile) return '';
    if (unit === 'mm') {
      const w = (profile.trim_width_in * 25.4).toFixed(0);
      const h = (profile.trim_height_in * 25.4).toFixed(0);
      return `${w}\u00d7${h}mm`;
    }
    return `${profile.trim_width_in}\u2033\u00d7${profile.trim_height_in}\u2033`;
  });
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Print Layout</h4>

  <div class="setting-group">
    <div class="group-label">Margins</div>

    {#if unitLoaded}
      {#key unit}
        <div class="margin-grid">
          <label class="margin-field">
            <span>Top</span>
            <div class="input-wrap">
              <input type="number" {step} min="0"
                value={toDisplay(topIn)}
                oninput={(e) => handleInput('top', e)} />
              <span class="unit-suffix">{uLabel}</span>
            </div>
          </label>

          <label class="margin-field">
            <span>Bottom</span>
            <div class="input-wrap">
              <input type="number" {step} min="0"
                value={toDisplay(bottomIn)}
                oninput={(e) => handleInput('bottom', e)} />
              <span class="unit-suffix">{uLabel}</span>
            </div>
          </label>

          <label class="margin-field">
            <span>Outside</span>
            <div class="input-wrap">
              <input type="number" {step} min="0"
                value={toDisplay(outsideIn)}
                oninput={(e) => handleInput('outside', e)} />
              <span class="unit-suffix">{uLabel}</span>
            </div>
          </label>

          <label class="margin-field">
            <span>Inside <span class="hint">(gutter)</span></span>
            <div class="input-wrap">
              <input type="number" {step} min="0"
                value={toDisplay(insideIn)}
                oninput={(e) => handleInput('inside', e)} />
              <span class="unit-suffix">{uLabel}</span>
            </div>
          </label>
        </div>
      {/key}
    {/if}

    <button class="reset-btn" onclick={resetToRecommended}
      title="Apply professional defaults for this trim size">
      <i class="bi bi-arrow-counterclockwise"></i>
      Reset to recommended for {trimLabel}
    </button>
  </div>

  <div class="setting-group">
    <div class="group-label">Text Flow</div>

    <button class="toggle-row" onclick={toggleJustify}>
      <span class="toggle-switch" class:on={justify}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Justified</span>
        <span class="toggle-hint">Stretch text to fill both margins (standard for books)</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleHyphens}>
      <span class="toggle-switch" class:on={hyphens}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Hyphenation</span>
        <span class="toggle-hint">Break words at line ends to avoid large gaps</span>
      </div>
    </button>
  </div>

  <div class="setting-group">
    <div class="group-label">Bleed</div>

    <button class="toggle-row" onclick={toggleBleed}>
      <span class="toggle-switch" class:on={bleedEnabled}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Enable bleed</span>
        <span class="toggle-hint">Extend page area beyond trim for edge-to-edge printing</span>
      </div>
    </button>

    {#if bleedEnabled && unitLoaded}
      {#key unit}
        <div class="bleed-amount">
          <label class="margin-field">
            <span>Bleed amount</span>
            <div class="input-wrap">
              <input type="number" step={step} min="0"
                value={toDisplay(bleedIn)}
                oninput={handleBleedInput} />
              <span class="unit-suffix">{uLabel}</span>
            </div>
          </label>
          <span class="bleed-hint">Standard: {unit === 'mm' ? '3mm' : '0.125"'} per side. Check your printer's requirements.</span>
        </div>
      {/key}
    {/if}
  </div>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }
  .setting-group { margin-bottom: 1rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.5rem;
  }

  .margin-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem;
  }
  .margin-field {
    display: flex; flex-direction: column; gap: 0.25rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted);
  }
  .margin-field > span:first-child {
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-weight: 600;
  }
  .hint {
    text-transform: none;
    font-weight: 400;
    opacity: 0.7;
    font-size: 0.68rem;
  }
  .input-wrap {
    display: flex; align-items: center;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    overflow: hidden;
  }
  .input-wrap:focus-within { border-color: var(--iwe-accent); }
  .input-wrap input {
    flex: 1; min-width: 0;
    border: none; background: none;
    padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text);
    outline: none;
  }
  /* Hide the spinner arrows — users type or use drag; the arrows are clutter */
  .input-wrap input::-webkit-outer-spin-button,
  .input-wrap input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .unit-suffix {
    padding: 0 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border-left: 1px solid var(--iwe-border);
    height: 100%;
    display: flex; align-items: center;
  }

  .reset-btn {
    margin-top: 0.8rem;
    width: 100%;
    display: flex; align-items: center; justify-content: center; gap: 0.4rem;
    padding: 0.5rem 0.7rem;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted);
    background: none;
    border: 1px dashed var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    cursor: pointer;
    transition: all 120ms;
  }
  .reset-btn:hover {
    color: var(--iwe-accent);
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.05);
  }
  .reset-btn i { font-size: 0.85rem; }

  /* Toggle rows */
  .toggle-row {
    width: 100%;
    display: flex; align-items: flex-start; gap: 0.7rem;
    padding: 0.55rem 0.1rem;
    background: none; border: none; cursor: pointer;
    text-align: left;
    border-bottom: 1px solid transparent;
    transition: background 100ms;
  }
  .toggle-row:hover { background: var(--iwe-bg-hover); }
  .toggle-row + .toggle-row {
    border-top: 1px solid var(--iwe-border);
  }
  .toggle-switch {
    flex-shrink: 0;
    width: 32px; height: 18px;
    border-radius: 9px;
    background: var(--iwe-border);
    position: relative;
    transition: background 150ms;
    margin-top: 2px;
  }
  .toggle-switch.on { background: var(--iwe-accent); }
  .toggle-knob {
    position: absolute;
    top: 2px; left: 2px;
    width: 14px; height: 14px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0,0,0,0.25);
    transition: transform 150ms;
  }
  .toggle-switch.on .toggle-knob { transform: translateX(14px); }
  .toggle-text {
    display: flex; flex-direction: column; gap: 0.15rem; min-width: 0;
  }
  .toggle-label {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .toggle-hint {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); line-height: 1.35;
  }

  .bleed-amount {
    padding: 0.5rem 0 0 0;
    display: flex; flex-direction: column; gap: 0.4rem;
    max-width: 180px;
  }
  .bleed-hint {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); line-height: 1.35;
  }
</style>
