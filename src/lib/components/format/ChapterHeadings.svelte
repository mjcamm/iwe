<script>
  import { updateProfileCategory } from '$lib/db.js';
  import FontPicker from '$lib/components/FontPicker.svelte';

  let { profile, onchange } = $props();

  const STYLES = [
    { id: 'regular', label: 'Regular' },
    { id: 'bold', label: 'Bold' },
    { id: 'italic', label: 'Italic' },
    { id: 'smallcaps', label: 'Small Caps' },
    { id: 'uppercase', label: 'Uppercase' },
  ];

  const NUMBER_FORMATS = [
    { id: 'none', label: 'No number' },
    { id: 'numeric', label: '1' },
    { id: 'chapter_numeric', label: 'Chapter 1' },
    { id: 'word', label: 'One' },
    { id: 'chapter_word', label: 'Chapter One' },
    { id: 'roman', label: 'I' },
    { id: 'chapter_roman', label: 'Chapter I' },
  ];

  const ALIGNS = [
    { id: 'left', label: 'Left' },
    { id: 'center', label: 'Center' },
    { id: 'right', label: 'Right' },
  ];

  const START_OPTIONS = [
    { id: 'any', label: 'Any page' },
    { id: 'recto', label: 'Right page (recto) only' },
  ];

  function defaults() {
    return {
      // Chapter number
      number_enabled: true,
      number_format: 'chapter_numeric',
      number_font: '',
      number_size_pt: 14,
      number_align: 'center',
      number_style: 'uppercase',
      number_tracking_em: 0,

      // Chapter title
      title_enabled: true,
      title_font: '',
      title_size_pt: 18,
      title_align: 'center',
      title_style: 'regular',
      title_tracking_em: 0,

      // Chapter subtitle
      subtitle_enabled: true,
      subtitle_font: '',
      subtitle_size_pt: 12,
      subtitle_align: 'center',
      subtitle_style: 'italic',
      subtitle_tracking_em: 0,

      // Spacing
      sink_em: 6,
      space_number_title_em: 1.5,
      space_title_subtitle_em: 0.8,
      space_after_heading_em: 3,

      // Page behavior
      start_on: 'any',

      // Rules
      rule_above: false,
      rule_below: false,
      rule_thickness_pt: 0.5,

      // Chapter image
      image_enabled: false,
      image_individual: true,
      image_default: '',
      image_position: 'below_heading',
      image_width_pct: 50,
      image_align: 'center',
      image_light_text: false,
    };
  }

  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.chapter_headings_json || '{}');
      return { ...defaults(), ...parsed };
    } catch {
      return defaults();
    }
  });

  // Local state
  let numEnabled = $state(true);
  let numFormat = $state('chapter_numeric');
  let numFont = $state('');
  let numSize = $state(14);
  let numAlign = $state('center');
  let numStyle = $state('uppercase');
  let numTracking = $state(0);

  let titleEnabled = $state(true);
  let titleFont = $state('');
  let titleSize = $state(18);
  let titleAlign = $state('center');
  let titleStyle = $state('regular');
  let titleTracking = $state(0);

  let subEnabled = $state(true);
  let subFont = $state('');
  let subSize = $state(12);
  let subAlign = $state('center');
  let subStyle = $state('italic');
  let subTracking = $state(0);

  let sinkEm = $state(6);
  let spaceNumTitle = $state(1.5);
  let spaceTitleSub = $state(0.8);
  let spaceAfter = $state(3);
  let startOn = $state('any');
  let ruleAbove = $state(false);
  let ruleBelow = $state(false);
  let ruleThickness = $state(0.5);

  let imgEnabled = $state(false);
  let imgIndividual = $state(true);
  let imgDefault = $state('');
  let imgPosition = $state('below_heading');
  let imgWidthPct = $state(50);
  let imgAlign = $state('center');
  let imgLightText = $state(false);

  let fileInputDefault;

  $effect(() => {
    numEnabled = settings.number_enabled;
    numFormat = settings.number_format;
    numFont = settings.number_font;
    numSize = settings.number_size_pt;
    numAlign = settings.number_align;
    numStyle = settings.number_style;
    numTracking = settings.number_tracking_em;
    titleEnabled = settings.title_enabled;
    titleFont = settings.title_font;
    titleSize = settings.title_size_pt;
    titleAlign = settings.title_align;
    titleStyle = settings.title_style;
    titleTracking = settings.title_tracking_em;
    subEnabled = settings.subtitle_enabled;
    subFont = settings.subtitle_font;
    subSize = settings.subtitle_size_pt;
    subAlign = settings.subtitle_align;
    subStyle = settings.subtitle_style;
    subTracking = settings.subtitle_tracking_em;
    sinkEm = settings.sink_em;
    spaceNumTitle = settings.space_number_title_em;
    spaceTitleSub = settings.space_title_subtitle_em;
    spaceAfter = settings.space_after_heading_em;
    startOn = settings.start_on;
    ruleAbove = settings.rule_above;
    ruleBelow = settings.rule_below;
    ruleThickness = settings.rule_thickness_pt;
    imgEnabled = settings.image_enabled;
    imgIndividual = settings.image_individual;
    imgDefault = settings.image_default;
    imgPosition = settings.image_position;
    imgWidthPct = settings.image_width_pct;
    imgAlign = settings.image_align;
    imgLightText = settings.image_light_text;
  });

  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      number_enabled: numEnabled, number_format: numFormat, number_font: numFont,
      number_size_pt: numSize, number_align: numAlign, number_style: numStyle,
      number_tracking_em: numTracking,
      title_enabled: titleEnabled, title_font: titleFont, title_size_pt: titleSize,
      title_align: titleAlign, title_style: titleStyle, title_tracking_em: titleTracking,
      subtitle_enabled: subEnabled, subtitle_font: subFont, subtitle_size_pt: subSize,
      subtitle_align: subAlign, subtitle_style: subStyle, subtitle_tracking_em: subTracking,
      sink_em: sinkEm, space_number_title_em: spaceNumTitle,
      space_title_subtitle_em: spaceTitleSub, space_after_heading_em: spaceAfter,
      start_on: startOn, rule_above: ruleAbove, rule_below: ruleBelow,
      rule_thickness_pt: ruleThickness,
      image_enabled: imgEnabled, image_individual: imgIndividual,
      image_default: imgDefault, image_position: imgPosition,
      image_width_pct: imgWidthPct, image_align: imgAlign,
      image_light_text: imgLightText,
    });
    await updateProfileCategory(profile.id, 'chapter_headings_json', json);
    onchange?.();
  }
</script>

<div class="custom-section">
  <h4 class="section-title">Chapter Headings</h4>

  <!-- ============ CHAPTER NUMBER ============ -->
  <div class="element-group">
    <button class="element-header" onclick={() => { numEnabled = !numEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={numEnabled}><span class="toggle-knob"></span></span>
      <span class="element-name">Chapter Number</span>
    </button>

    {#if numEnabled}
      <div class="element-settings">
        <div class="field">
          <span class="field-label">Format</span>
          <select class="field-select" bind:value={numFormat} onchange={scheduleSave}>
            {#each NUMBER_FORMATS as nf}<option value={nf.id}>{nf.label}</option>{/each}
          </select>
        </div>
        <div class="field">
          <span class="field-label">Font</span>
          <FontPicker bind:value={numFont} onchange={(f) => { numFont = f; scheduleSave(); }} placeholder="Default" />
        </div>
        <div class="row-3col">
          <div class="field">
            <span class="field-label">Size</span>
            <select class="field-select" bind:value={numSize} onchange={scheduleSave}>
              {#each [9,10,11,12,14,16,18,20,24,28,32,36,42] as s}<option value={s}>{s} pt</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Align</span>
            <select class="field-select" bind:value={numAlign} onchange={scheduleSave}>
              {#each ALIGNS as a}<option value={a.id}>{a.label}</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Style</span>
            <select class="field-select" bind:value={numStyle} onchange={scheduleSave}>
              {#each STYLES as s}<option value={s.id}>{s.label}</option>{/each}
            </select>
          </div>
        </div>
        <div class="field">
          <span class="field-label">Letter spacing — {numTracking}em</span>
          <input type="range" class="slider" min="0" max="0.5" step="0.02"
            bind:value={numTracking} oninput={scheduleSave} />
        </div>
      </div>
    {/if}
  </div>

  <!-- ============ CHAPTER TITLE ============ -->
  <div class="element-group">
    <button class="element-header" onclick={() => { titleEnabled = !titleEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={titleEnabled}><span class="toggle-knob"></span></span>
      <span class="element-name">Chapter Title</span>
    </button>

    {#if titleEnabled}
      <div class="element-settings">
        <div class="field">
          <span class="field-label">Font</span>
          <FontPicker bind:value={titleFont} onchange={(f) => { titleFont = f; scheduleSave(); }} placeholder="Default" />
        </div>
        <div class="row-3col">
          <div class="field">
            <span class="field-label">Size</span>
            <select class="field-select" bind:value={titleSize} onchange={scheduleSave}>
              {#each [12,14,16,18,20,24,28,32,36,42,48,54] as s}<option value={s}>{s} pt</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Align</span>
            <select class="field-select" bind:value={titleAlign} onchange={scheduleSave}>
              {#each ALIGNS as a}<option value={a.id}>{a.label}</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Style</span>
            <select class="field-select" bind:value={titleStyle} onchange={scheduleSave}>
              {#each STYLES as s}<option value={s.id}>{s.label}</option>{/each}
            </select>
          </div>
        </div>
        <div class="field">
          <span class="field-label">Letter spacing — {titleTracking}em</span>
          <input type="range" class="slider" min="0" max="0.5" step="0.02"
            bind:value={titleTracking} oninput={scheduleSave} />
        </div>
      </div>
    {/if}
  </div>

  <!-- ============ CHAPTER SUBTITLE ============ -->
  <div class="element-group">
    <button class="element-header" onclick={() => { subEnabled = !subEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={subEnabled}><span class="toggle-knob"></span></span>
      <span class="element-name">Chapter Subtitle</span>
    </button>

    {#if subEnabled}
      <div class="element-settings">
        <div class="field">
          <span class="field-label">Font</span>
          <FontPicker bind:value={subFont} onchange={(f) => { subFont = f; scheduleSave(); }} placeholder="Default" />
        </div>
        <div class="row-3col">
          <div class="field">
            <span class="field-label">Size</span>
            <select class="field-select" bind:value={subSize} onchange={scheduleSave}>
              {#each [9,10,11,12,14,16,18,20,24,28,32] as s}<option value={s}>{s} pt</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Align</span>
            <select class="field-select" bind:value={subAlign} onchange={scheduleSave}>
              {#each ALIGNS as a}<option value={a.id}>{a.label}</option>{/each}
            </select>
          </div>
          <div class="field">
            <span class="field-label">Style</span>
            <select class="field-select" bind:value={subStyle} onchange={scheduleSave}>
              {#each STYLES as s}<option value={s.id}>{s.label}</option>{/each}
            </select>
          </div>
        </div>
        <div class="field">
          <span class="field-label">Letter spacing — {subTracking}em</span>
          <input type="range" class="slider" min="0" max="0.5" step="0.02"
            bind:value={subTracking} oninput={scheduleSave} />
        </div>
      </div>
    {/if}
  </div>

  <!-- ============ SPACING ============ -->
  <div class="setting-group">
    <div class="group-label">Spacing</div>
    <div class="row-2col">
      <label class="field">
        <span class="field-label">Chapter sink</span>
        <span class="field-hint">How far down the page before the heading starts</span>
        <div class="input-row">
          <input type="number" step="0.5" min="0" max="20" class="field-num"
            bind:value={sinkEm} oninput={scheduleSave} />
          <span class="unit">em</span>
        </div>
      </label>
      <label class="field">
        <span class="field-label">After heading</span>
        <span class="field-hint">Gap between the heading block and the first paragraph</span>
        <div class="input-row">
          <input type="number" step="0.5" min="0" max="10" class="field-num"
            bind:value={spaceAfter} oninput={scheduleSave} />
          <span class="unit">em</span>
        </div>
      </label>
    </div>
    {#if (numEnabled && titleEnabled) || (titleEnabled && subEnabled)}
      <div class="row-2col">
        {#if numEnabled && titleEnabled}
          <label class="field">
            <span class="field-label">Number → Title</span>
            <span class="field-hint">Space between the chapter number and the title</span>
            <div class="input-row">
              <input type="number" step="0.25" min="0" max="5" class="field-num"
                bind:value={spaceNumTitle} oninput={scheduleSave} />
              <span class="unit">em</span>
            </div>
          </label>
        {/if}
        {#if titleEnabled && subEnabled}
          <label class="field">
            <span class="field-label">Title → Subtitle</span>
            <span class="field-hint">Space between the title and the subtitle</span>
            <div class="input-row">
              <input type="number" step="0.25" min="0" max="5" class="field-num"
                bind:value={spaceTitleSub} oninput={scheduleSave} />
              <span class="unit">em</span>
            </div>
          </label>
        {/if}
      </div>
    {/if}
  </div>

  <!-- ============ PAGE BEHAVIOR ============ -->
  <div class="setting-group">
    <div class="group-label">Page Behavior</div>
    <div class="field">
      <span class="field-label">Start chapters on</span>
      <select class="field-select" bind:value={startOn} onchange={scheduleSave}>
        {#each START_OPTIONS as opt}<option value={opt.id}>{opt.label}</option>{/each}
      </select>
    </div>
  </div>

  <!-- ============ RULES ============ -->
  <div class="setting-group">
    <div class="group-label">Rules</div>
    <button class="toggle-row" onclick={() => { ruleAbove = !ruleAbove; scheduleSave(); }}>
      <span class="toggle-switch" class:on={ruleAbove}><span class="toggle-knob"></span></span>
      <span class="toggle-label">Rule above heading</span>
    </button>
    <button class="toggle-row" onclick={() => { ruleBelow = !ruleBelow; scheduleSave(); }}>
      <span class="toggle-switch" class:on={ruleBelow}><span class="toggle-knob"></span></span>
      <span class="toggle-label">Rule below heading</span>
    </button>
    {#if ruleAbove || ruleBelow}
      <label class="field" style="margin-top: 0.4rem;">
        <span class="field-label">Thickness</span>
        <div class="input-row">
          <input type="number" step="0.25" min="0.25" max="3" class="field-num"
            bind:value={ruleThickness} oninput={scheduleSave} />
          <span class="unit">pt</span>
        </div>
      </label>
    {/if}
  </div>
  <!-- ============ CHAPTER IMAGE ============ -->
  <div class="setting-group">
    <div class="group-label">Chapter Image</div>

    <button class="toggle-row" onclick={() => { imgEnabled = !imgEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={imgEnabled}><span class="toggle-knob"></span></span>
      <span class="toggle-label">Enable chapter images</span>
    </button>

    {#if imgEnabled}
      <div class="sub-settings">
        <div class="field">
          <span class="field-label">Image source</span>
          <select class="field-select" bind:value={imgIndividual} onchange={scheduleSave}>
            <option value={true}>Use individual chapter images</option>
            <option value={false}>Use a single image for all chapters</option>
          </select>
        </div>

        {#if !imgIndividual}
          <div class="field">
            <span class="field-label">Image for all chapters</span>
            {#if imgDefault}
              <div class="img-preview">
                <img src={imgDefault} alt="Default" />
                <div class="img-actions">
                  <button class="img-btn" onclick={() => fileInputDefault?.click()}>Replace</button>
                  <button class="img-btn danger" onclick={() => { imgDefault = ''; scheduleSave(); }}>Remove</button>
                </div>
              </div>
            {:else}
              <button class="img-upload" onclick={() => fileInputDefault?.click()}>
                <i class="bi bi-image"></i> Upload image
              </button>
            {/if}
            <input type="file" accept="image/png,image/svg+xml,image/jpeg"
              bind:this={fileInputDefault}
              onchange={(e) => {
                const f = e.target.files?.[0];
                if (!f) return;
                const r = new FileReader();
                r.onload = () => { imgDefault = r.result; scheduleSave(); };
                r.readAsDataURL(f);
                e.target.value = '';
              }}
              style="display:none" />
          </div>
        {/if}

        <div class="field">
          <span class="field-label">Position</span>
          <select class="field-select" bind:value={imgPosition} onchange={scheduleSave}>
            {#if numEnabled}
              <option value="above_number">Above chapter number</option>
            {/if}
            {#if numEnabled && titleEnabled}
              <option value="between_number_title">Between number and title</option>
            {/if}
            {#if titleEnabled && subEnabled}
              <option value="between_title_subtitle">Between title and subtitle</option>
            {/if}
            <option value="below_heading">Below heading block</option>
            <option value="cover_heading">Cover entire heading area</option>
            <option value="cover_page">Cover entire page</option>
          </select>
        </div>

        <div class="field">
          <span class="field-label">Width — {imgWidthPct}%</span>
          <input type="range" class="slider" min="5" max="150" step="1"
            bind:value={imgWidthPct} oninput={scheduleSave} />
        </div>

        <div class="field">
          <span class="field-label">Alignment</span>
          <select class="field-select" bind:value={imgAlign} onchange={scheduleSave}>
            <option value="left">Left</option>
            <option value="center">Center</option>
            <option value="right">Right</option>
          </select>
        </div>

        {#if imgPosition === 'cover_page' || imgPosition === 'cover_heading'}
          <button class="toggle-row" onclick={() => { imgLightText = !imgLightText; scheduleSave(); }}>
            <span class="toggle-switch" class:on={imgLightText}><span class="toggle-knob"></span></span>
            <div class="toggle-text">
              <span class="toggle-label">Light text (white)</span>
              <span class="field-hint">Use white text over dark images</span>
            </div>
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }

  /* Element groups (number/title/subtitle) */
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
  .element-name {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .element-settings {
    padding: 0.6rem 0.7rem;
    display: flex; flex-direction: column; gap: 0.6rem;
    border-top: 1px solid var(--iwe-border);
  }

  /* Setting groups */
  .setting-group { margin-bottom: 0.8rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.4rem;
  }

  /* Fields */
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
  .field-hint {
    font-family: var(--iwe-font-ui); font-size: 0.62rem;
    color: var(--iwe-text-muted); font-style: italic;
    font-weight: 400; text-transform: none; letter-spacing: 0;
    line-height: 1.3;
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

  .row-2col { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; margin-bottom: 0.5rem; }
  .row-3col { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 0.4rem; }
  .input-row { display: flex; align-items: center; gap: 0.3rem; }
  .unit {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted);
  }
  .slider { width: 100%; accent-color: var(--iwe-accent); }
  .sub-settings {
    display: flex; flex-direction: column; gap: 0.6rem;
    padding-top: 0.4rem;
  }

  /* Image controls */
  .img-preview {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.5rem; background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
  }
  .img-preview img {
    display: block; max-width: 100%; max-height: 100px;
    margin: 0 auto; background: #fff; padding: 0.3rem;
    border-radius: 3px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }
  .img-actions { display: flex; gap: 0.3rem; }
  .img-btn {
    flex: 1; padding: 0.3rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); color: var(--iwe-text);
    cursor: pointer; transition: all 100ms;
  }
  .img-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .img-btn.danger:hover { border-color: #c0392b; color: #c0392b; }
  .img-upload {
    display: flex; align-items: center; justify-content: center; gap: 0.4rem;
    padding: 0.6rem; font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); background: var(--iwe-bg-warm);
    border: 1px dashed var(--iwe-border); border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .img-upload:hover { color: var(--iwe-accent); border-color: var(--iwe-accent); }

  /* Toggles */
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
  .toggle-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text);
  }
</style>
