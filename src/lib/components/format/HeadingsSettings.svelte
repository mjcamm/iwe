<script>
  import { updateProfileCategory } from '$lib/db.js';
  import FontPicker from '$lib/components/FontPicker.svelte';

  let { profile, onchange } = $props();
  let isPrint = $derived(profile?.target_type !== 'ebook');

  const STYLES = [
    { id: 'regular', label: 'Regular' },
    { id: 'bold', label: 'Bold' },
    { id: 'italic', label: 'Italic' },
    { id: 'smallcaps', label: 'Small Caps' },
    { id: 'uppercase', label: 'Uppercase' },
  ];

  const ALIGNS = [
    { id: 'left', label: 'Left' },
    { id: 'center', label: 'Center' },
    { id: 'right', label: 'Right' },
  ];

  const LEVELS = [
    { key: 'h2', label: 'Heading 2', hint: 'Major section dividers within a chapter', defaultSize: 16, defaultStyle: 'bold' },
    { key: 'h3', label: 'Heading 3', hint: 'Sub-sections', defaultSize: 13, defaultStyle: 'bold' },
    { key: 'h4', label: 'Heading 4', hint: 'Minor divisions', defaultSize: 11, defaultStyle: 'italic' },
  ];

  function defaults() {
    const d = {
      no_indent_after: true,
    };
    for (const lvl of LEVELS) {
      d[`${lvl.key}_enabled`] = true;
      d[`${lvl.key}_font`] = '';
      d[`${lvl.key}_size_pt`] = lvl.defaultSize;
      d[`${lvl.key}_align`] = 'left';
      d[`${lvl.key}_style`] = lvl.defaultStyle;
      d[`${lvl.key}_tracking_em`] = 0;
      d[`${lvl.key}_space_above_em`] = 1.5;
      d[`${lvl.key}_space_below_em`] = 0.8;
      d[`${lvl.key}_keep_with_next`] = true;
      d[`${lvl.key}_rule_above`] = false;
      d[`${lvl.key}_rule_below`] = false;
    }
    return d;
  }

  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.headings_json || '{}');
      return { ...defaults(), ...parsed };
    } catch {
      return defaults();
    }
  });

  // Dynamic state per heading level — store as a flat object
  let state = $state({});

  $effect(() => {
    state = { ...settings };
  });

  function get(key) { return state[key]; }
  function set(key, val) { state[key] = val; state = { ...state }; scheduleSave(); }

  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  async function persist() {
    if (!profile) return;
    await updateProfileCategory(profile.id, 'headings_json', JSON.stringify(state));
    onchange?.();
  }
</script>

<div class="custom-section">
  <h4 class="section-title">In-Chapter Headings</h4>
  <p class="section-hint">Style for subheadings within chapter body text (H2, H3, H4).</p>

  {#each LEVELS as lvl}
    {@const enabled = get(`${lvl.key}_enabled`)}
    <div class="element-group">
      <button class="element-header" onclick={() => set(`${lvl.key}_enabled`, !enabled)}>
        <span class="toggle-switch" class:on={enabled}><span class="toggle-knob"></span></span>
        <div class="element-header-text">
          <span class="element-name">{lvl.label}</span>
          <span class="element-hint">{lvl.hint}</span>
        </div>
      </button>

      {#if enabled}
        <div class="element-settings">
          <div class="field">
            <span class="field-label">Font</span>
            <FontPicker value={get(`${lvl.key}_font`)}
              onchange={(f) => set(`${lvl.key}_font`, f)}
              placeholder="Default (body font)" />
          </div>

          <div class="row-3col">
            <div class="field">
              <span class="field-label">Size</span>
              <select class="field-select" value={get(`${lvl.key}_size_pt`)}
                onchange={(e) => set(`${lvl.key}_size_pt`, Number(e.target.value))}>
                {#each [9,10,11,12,13,14,16,18,20,24] as s}<option value={s}>{s} pt</option>{/each}
              </select>
            </div>
            <div class="field">
              <span class="field-label">Align</span>
              <select class="field-select" value={get(`${lvl.key}_align`)}
                onchange={(e) => set(`${lvl.key}_align`, e.target.value)}>
                {#each ALIGNS as a}<option value={a.id}>{a.label}</option>{/each}
              </select>
            </div>
            <div class="field">
              <span class="field-label">Style</span>
              <select class="field-select" value={get(`${lvl.key}_style`)}
                onchange={(e) => set(`${lvl.key}_style`, e.target.value)}>
                {#each STYLES as s}<option value={s.id}>{s.label}</option>{/each}
              </select>
            </div>
          </div>

          <div class="field">
            <span class="field-label">Letter spacing — {get(`${lvl.key}_tracking_em`)}em</span>
            <input type="range" class="slider" min="0" max="0.5" step="0.02"
              value={get(`${lvl.key}_tracking_em`)}
              oninput={(e) => set(`${lvl.key}_tracking_em`, Number(e.target.value))} />
          </div>

          <div class="row-2col">
            <label class="field">
              <span class="field-label">Space above</span>
              <div class="input-row">
                <input type="number" step="0.25" min="0" max="5" class="field-num"
                  value={get(`${lvl.key}_space_above_em`)}
                  oninput={(e) => set(`${lvl.key}_space_above_em`, Number(e.target.value))} />
                <span class="unit">em</span>
              </div>
            </label>
            <label class="field">
              <span class="field-label">Space below</span>
              <div class="input-row">
                <input type="number" step="0.25" min="0" max="5" class="field-num"
                  value={get(`${lvl.key}_space_below_em`)}
                  oninput={(e) => set(`${lvl.key}_space_below_em`, Number(e.target.value))} />
                <span class="unit">em</span>
              </div>
            </label>
          </div>

          {#if isPrint}
          <button class="toggle-row" onclick={() => set(`${lvl.key}_keep_with_next`, !get(`${lvl.key}_keep_with_next`))}>
            <span class="toggle-switch" class:on={get(`${lvl.key}_keep_with_next`)}><span class="toggle-knob"></span></span>
            <div class="toggle-text">
              <span class="toggle-label">Keep with next paragraph</span>
              <span class="toggle-hint">Prevent heading from being orphaned at the bottom of a page</span>
            </div>
          </button>
          {/if}

          <div class="row-2col">
            <button class="toggle-row" onclick={() => set(`${lvl.key}_rule_above`, !get(`${lvl.key}_rule_above`))}>
              <span class="toggle-switch" class:on={get(`${lvl.key}_rule_above`)}><span class="toggle-knob"></span></span>
              <span class="toggle-label">Rule above</span>
            </button>
            <button class="toggle-row" onclick={() => set(`${lvl.key}_rule_below`, !get(`${lvl.key}_rule_below`))}>
              <span class="toggle-switch" class:on={get(`${lvl.key}_rule_below`)}><span class="toggle-knob"></span></span>
              <span class="toggle-label">Rule below</span>
            </button>
          </div>
        </div>
      {/if}
    </div>
  {/each}

  <!-- Global -->
  <div class="setting-group">
    <div class="group-label">Paragraph Behavior</div>
    <button class="toggle-row" onclick={() => set('no_indent_after', !get('no_indent_after'))}>
      <span class="toggle-switch" class:on={get('no_indent_after')}><span class="toggle-knob"></span></span>
      <div class="toggle-text">
        <span class="toggle-label">No indent after heading</span>
        <span class="toggle-hint">First paragraph after a subheading starts flush left</span>
      </div>
    </button>
  </div>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.3rem 0; color: var(--iwe-text);
  }
  .section-hint {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); margin: 0 0 0.8rem 0;
  }

  .element-group {
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    margin-bottom: 0.6rem;
  }
  .element-header {
    width: 100%;
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.5rem 0.7rem;
    border: none; background: var(--iwe-bg-warm);
    border-radius: var(--iwe-radius-sm) var(--iwe-radius-sm) 0 0;
    cursor: pointer; text-align: left;
    transition: background 100ms;
  }
  .element-header:hover { background: var(--iwe-bg-hover); }
  .element-header-text { display: flex; flex-direction: column; gap: 0.1rem; }
  .element-name {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .element-hint {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted);
  }
  .element-settings {
    padding: 0.6rem 0.7rem;
    display: flex; flex-direction: column; gap: 0.6rem;
    border-top: 1px solid var(--iwe-border);
  }

  .setting-group { margin-bottom: 0.8rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.4rem;
  }

  .field { display: flex; flex-direction: column; gap: 0.2rem; }
  .field-label {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
  }
  .field-select {
    padding: 0.35rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
  }
  .field-select:focus { outline: none; border-color: var(--iwe-accent); }
  .field-num {
    flex: 1; min-width: 0; padding: 0.35rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .field-num:focus { outline: none; border-color: var(--iwe-accent); }
  .field-num::-webkit-outer-spin-button,
  .field-num::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .row-2col { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .row-3col { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 0.4rem; }
  .input-row { display: flex; align-items: center; gap: 0.3rem; }
  .unit { font-family: var(--iwe-font-ui); font-size: 0.7rem; color: var(--iwe-text-muted); }
  .slider { width: 100%; accent-color: var(--iwe-accent); }

  .toggle-row {
    width: 100%;
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.4rem 0.1rem;
    background: none; border: none; cursor: pointer;
    text-align: left;
  }
  .toggle-row:hover { background: var(--iwe-bg-hover); }
  .toggle-switch {
    flex-shrink: 0; width: 32px; height: 18px;
    border-radius: 9px; background: var(--iwe-border);
    position: relative; transition: background 150ms;
  }
  .toggle-switch.on { background: var(--iwe-accent); }
  .toggle-knob {
    position: absolute; top: 2px; left: 2px;
    width: 14px; height: 14px; border-radius: 50%;
    background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,0.25);
    transition: transform 150ms;
  }
  .toggle-switch.on .toggle-knob { transform: translateX(14px); }
  .toggle-text { display: flex; flex-direction: column; gap: 0.1rem; min-width: 0; }
  .toggle-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text);
  }
  .toggle-hint {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted);
  }
</style>
