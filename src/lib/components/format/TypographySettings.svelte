<script>
  import FontPicker from '$lib/components/FontPicker.svelte';
  import { updateProfileCategory } from '$lib/db.js';

  let { profile, onchange } = $props();

  // Parse the typography_json column with sensible fallbacks to legacy scalar fields.
  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.typography_json || '{}');
      return {
        font: parsed.font ?? profile?.font_body ?? 'Liberation Serif',
        size_pt: parsed.size_pt ?? profile?.font_size_pt ?? 11,
        line_spacing: parsed.line_spacing ?? profile?.line_spacing ?? 1.4,
      };
    } catch {
      return { font: 'Liberation Serif', size_pt: 11, line_spacing: 1.4 };
    }
  });

  // Local working copy
  let font = $state(settings.font);
  let sizePt = $state(settings.size_pt);
  let lineSpacing = $state(settings.line_spacing);

  // When the profile prop changes (e.g. switching profiles), reset the local state
  $effect(() => {
    font = settings.font;
    sizePt = settings.size_pt;
    lineSpacing = settings.line_spacing;
  });

  const FONT_SIZES = [9, 10, 11, 12, 13, 14, 15, 16, 17, 18];
  // Line spacing: multiplier of font size (baseline-to-baseline distance).
  // "Single" matches the Word convention (~1.15) rather than the mathematical 1.0,
  // which would set lines solid with no gap.
  const LINE_SPACINGS = [
    { label: 'Single',  value: 1.15 },
    { label: '1.25',    value: 1.25 },
    { label: '1.5',     value: 1.5 },
    { label: '1.75',    value: 1.75 },
    { label: 'Double',  value: 2.0 },
  ];

  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 200);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      font,
      size_pt: sizePt,
      line_spacing: lineSpacing,
    });
    await updateProfileCategory(profile.id, 'typography_json', json);
    onchange?.();
  }

  function onFontChange(newFont) {
    font = newFont;
    scheduleSave();
  }
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Typography</h4>

  <div class="setting-row">
    <label class="setting-label">Body font</label>
    <FontPicker bind:value={font} onchange={onFontChange} />
  </div>

  <div class="setting-row">
    <label class="setting-label">Font size</label>
    <select class="setting-select" bind:value={sizePt} onchange={scheduleSave}>
      {#each FONT_SIZES as s}
        <option value={s}>{s} pt</option>
      {/each}
    </select>
  </div>

  <div class="setting-row">
    <label class="setting-label">Line spacing</label>
    <select class="setting-select" bind:value={lineSpacing} onchange={scheduleSave}>
      {#each LINE_SPACINGS as opt}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
  </div>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }
  .setting-row {
    display: flex; flex-direction: column; gap: 0.3rem;
    margin-bottom: 0.9rem;
  }
  .setting-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .setting-select {
    width: 100%;
    padding: 0.4rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
  }
  .setting-select:focus { outline: none; border-color: var(--iwe-accent); }
</style>
