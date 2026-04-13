<script>
  import { updateProfileCategory } from '$lib/db.js';
  import DecimalInput from '$lib/components/DecimalInput.svelte';

  let { profile, onchange } = $props();
  let isPrint = $derived(profile?.target_type !== 'ebook');

  const STYLES = [
    { id: 'none',     label: 'None',             preview: '(nothing)' },
    { id: 'blank',    label: 'Blank line only',  preview: ' ' },
    { id: 'dinkus',   label: 'Dinkus',           preview: '* * *' },
    { id: 'asterism', label: 'Asterism',         preview: '⁂' },
    { id: 'rule',     label: 'Horizontal rule',  preview: '—————' },
    { id: 'custom',   label: 'Custom text',      preview: null },
    { id: 'image',    label: 'Image (PNG/SVG)',  preview: null },
  ];

  // Read from breaks_json with sensible defaults
  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.breaks_json || '{}');
      return {
        style: parsed.style ?? 'dinkus',
        custom_text: parsed.custom_text ?? '* * *',
        space_above_em: parsed.space_above_em ?? 1.2,
        space_below_em: parsed.space_below_em ?? 1.2,
        keep_with_content: parsed.keep_with_content ?? true,
        image_data: parsed.image_data ?? '',
        image_width_pct: parsed.image_width_pct ?? 25,
      };
    } catch {
      return {
        style: 'dinkus', custom_text: '* * *',
        space_above_em: 1.2, space_below_em: 1.2,
        keep_with_content: true, image_data: '', image_width_pct: 25,
      };
    }
  });

  let style = $state(settings.style);
  let customText = $state(settings.custom_text);
  let spaceAbove = $state(settings.space_above_em);
  let spaceBelow = $state(settings.space_below_em);
  let keepWithContent = $state(settings.keep_with_content);
  let imageData = $state(settings.image_data);
  let imageWidthPct = $state(settings.image_width_pct);

  $effect(() => {
    style = settings.style;
    customText = settings.custom_text;
    spaceAbove = settings.space_above_em;
    spaceBelow = settings.space_below_em;
    keepWithContent = settings.keep_with_content;
    imageData = settings.image_data;
    imageWidthPct = settings.image_width_pct;
  });

  let fileInput;

  function triggerUpload() {
    fileInput?.click();
  }

  function handleFileSelected(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!/^image\/(png|svg\+xml|jpeg|jpg)$/i.test(file.type)) {
      alert('Please upload a PNG, JPEG or SVG image.');
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      imageData = reader.result;
      scheduleSave();
    };
    reader.readAsDataURL(file);
    e.target.value = '';
  }

  function clearImage() {
    imageData = '';
    scheduleSave();
  }

  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      style,
      custom_text: customText,
      space_above_em: spaceAbove,
      space_below_em: spaceBelow,
      keep_with_content: keepWithContent,
      image_data: imageData,
      image_width_pct: imageWidthPct,
    });
    await updateProfileCategory(profile.id, 'breaks_json', json);
    onchange?.();
  }

  function selectStyle(newStyle) {
    style = newStyle;
    scheduleSave();
  }

  function toggleKeep() {
    keepWithContent = !keepWithContent;
    scheduleSave();
  }
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Breaks</h4>

  <div class="intro">
    Scene breaks appear within chapters to separate scenes or time jumps.
    Type <code>***</code> on a line by itself in the chapter editor — it will
    be replaced with the style you pick here.
  </div>

  <div class="setting-group">
    <div class="group-label">Style</div>
    <div class="style-list">
      {#each STYLES as opt}
        <button class="style-option" class:selected={style === opt.id}
          onclick={() => selectStyle(opt.id)}>
          <span class="style-label">{opt.label}</span>
          {#if opt.preview !== null}
            <span class="style-preview">{opt.preview}</span>
          {/if}
        </button>
      {/each}
    </div>

    {#if style === 'custom'}
      <div class="custom-text-row">
        <label class="field-label">Custom text</label>
        <input type="text" class="field-input"
          bind:value={customText}
          oninput={scheduleSave}
          placeholder="Enter any text or symbols" />
      </div>
    {/if}

    {#if style === 'image'}
      <div class="image-upload-row">
        <label class="field-label">Image</label>
        {#if imageData}
          <div class="image-preview-wrap">
            <img class="image-preview" src={imageData} alt="Scene break" />
            <div class="image-actions">
              <button class="image-action-btn" onclick={triggerUpload}>Replace</button>
              <button class="image-action-btn danger" onclick={clearImage}>Remove</button>
            </div>
          </div>
        {:else}
          <button class="image-upload-btn" onclick={triggerUpload}>
            <i class="bi bi-upload"></i>
            Upload PNG or SVG
          </button>
        {/if}
        <input type="file" accept="image/png,image/svg+xml,image/jpeg"
          bind:this={fileInput} onchange={handleFileSelected} style="display: none;" />

        <label class="field-label" style="margin-top: 0.8rem;">
          Width <span class="width-value">{imageWidthPct}% of text width</span>
        </label>
        <input type="range" class="width-slider"
          min="5" max="100" step="1"
          bind:value={imageWidthPct} oninput={scheduleSave} />
      </div>
    {/if}
  </div>

  <div class="setting-group">
    <div class="group-label">Spacing</div>
    <div class="spacing-grid">
      <div class="spacing-field">
        <span>Above</span>
        <DecimalInput value={spaceAbove} onchange={(v) => { spaceAbove = v; scheduleSave(); }}
          suffix="em" step={0.1} min={0} decimals={2} />
      </div>
      <div class="spacing-field">
        <span>Below</span>
        <DecimalInput value={spaceBelow} onchange={(v) => { spaceBelow = v; scheduleSave(); }}
          suffix="em" step={0.1} min={0} decimals={2} />
      </div>
    </div>
  </div>

  {#if isPrint}
  <div class="setting-group">
    <div class="group-label">Page Breaks</div>
    <button class="toggle-row" onclick={toggleKeep}>
      <span class="toggle-switch" class:on={keepWithContent}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Keep with following content</span>
        <span class="toggle-hint">Prevents scene breaks from landing at the top or bottom of a page where they're visually orphaned.</span>
      </div>
    </button>
  </div>
  {/if}
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.5rem 0; color: var(--iwe-text);
  }
  .intro {
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted); line-height: 1.5;
    padding: 0.5rem 0.7rem;
    background: var(--iwe-bg-warm);
    border-left: 2px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm);
    margin-bottom: 1rem;
  }
  .intro code {
    background: var(--iwe-bg);
    padding: 0.5px 5px;
    border-radius: 3px;
    font-family: monospace; font-size: 0.8rem;
    color: var(--iwe-accent);
  }

  .setting-group { margin-bottom: 1rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.5rem;
  }

  /* Style option list */
  .style-list {
    display: flex; flex-direction: column; gap: 3px;
  }
  .style-option {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.55rem 0.7rem;
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    cursor: pointer; transition: all 100ms;
    font-family: var(--iwe-font-ui);
  }
  .style-option:hover { border-color: var(--iwe-accent); }
  .style-option.selected {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
    color: var(--iwe-accent);
    font-weight: 500;
  }
  .style-label { font-size: 0.82rem; }
  .style-preview {
    font-family: 'Liberation Serif', Georgia, serif;
    font-size: 0.95rem;
    color: var(--iwe-text-muted);
    letter-spacing: 0.1em;
  }
  .style-option.selected .style-preview { color: var(--iwe-accent); }

  /* Custom text input */
  .custom-text-row { margin-top: 0.6rem; }
  .field-label {
    display: block;
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.25rem;
  }
  .field-input {
    width: 100%;
    padding: 0.4rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .field-input:focus { outline: none; border-color: var(--iwe-accent); }

  /* Image upload */
  .image-upload-row { margin-top: 0.6rem; }
  .image-upload-btn {
    width: 100%;
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    padding: 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border: 1px dashed var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .image-upload-btn:hover {
    color: var(--iwe-accent);
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.05);
  }
  .image-upload-btn i { font-size: 1rem; }

  .image-preview-wrap {
    display: flex; flex-direction: column; gap: 0.5rem;
    padding: 0.75rem;
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
  }
  .image-preview {
    display: block;
    max-width: 100%;
    max-height: 120px;
    margin: 0 auto;
    background: #fff;
    padding: 0.5rem;
    border-radius: 4px;
    box-shadow: 0 1px 4px rgba(0,0,0,0.08);
  }
  .image-actions {
    display: flex; gap: 0.4rem;
  }
  .image-action-btn {
    flex: 1;
    padding: 0.35rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
  }
  .image-action-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .image-action-btn.danger:hover { border-color: #c0392b; color: #c0392b; }

  .width-value {
    text-transform: none;
    font-weight: 400;
    color: var(--iwe-text);
    opacity: 0.8;
  }
  .width-slider {
    width: 100%;
    accent-color: var(--iwe-accent);
  }

  /* Spacing grid */
  .spacing-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem;
  }
  .spacing-field {
    display: flex; flex-direction: column; gap: 0.25rem;
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-weight: 600;
  }

  /* Toggle row */
  .toggle-row {
    width: 100%;
    display: flex; align-items: flex-start; gap: 0.7rem;
    padding: 0.55rem 0.1rem;
    background: none; border: none; cursor: pointer;
    text-align: left;
    transition: background 100ms;
  }
  .toggle-row:hover { background: var(--iwe-bg-hover); }
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
</style>
