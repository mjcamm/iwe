<script>
  import { onMount } from 'svelte';
  import { updateProfileCategory } from '$lib/db.js';
  import FontPicker from '$lib/components/FontPicker.svelte';
  import { ensureUnitLoaded, getUnit, toDisplay, fromDisplay, unitLabel, unitStep, subscribe } from '$lib/unitPreference.js';

  let { profile, onchange } = $props();

  let unit = $state('in');
  onMount(async () => {
    unit = await ensureUnitLoaded();
    return subscribe(u => { unit = u; });
  });
  let uLabel = $derived(unitLabel());
  let step = $derived(unitStep());

  const CONTENT_TYPES = [
    { id: 'none',          label: 'Empty' },
    { id: 'page_number',   label: 'Page Number' },
    { id: 'book_title',    label: 'Book Title' },
    { id: 'chapter_title', label: 'Chapter Title' },
    { id: 'author_name',   label: 'Author Name' },
    { id: 'series_name',   label: 'Series Name' },
    { id: 'book_number',   label: 'Book Number' },
    { id: 'custom',        label: 'Custom Text' },
  ];

  const TEXT_STYLES = [
    { id: 'normal',    label: 'Normal' },
    { id: 'italic',    label: 'Italic' },
    { id: 'smallcaps', label: 'Small Caps' },
    { id: 'uppercase', label: 'Uppercase' },
  ];

  // All 12 slot keys in a fixed order
  const SLOTS = [
    'verso_header_left', 'verso_header_center', 'verso_header_right',
    'verso_footer_left', 'verso_footer_center', 'verso_footer_right',
    'recto_header_left', 'recto_header_center', 'recto_header_right',
    'recto_footer_left', 'recto_footer_center', 'recto_footer_right',
  ];

  function defaultSlot() {
    return { content: 'none', custom: '', font: '', size_pt: 9, style: 'normal' };
  }

  function defaultSettings() {
    return {
      slots: Object.fromEntries(SLOTS.map(k => [k, defaultSlot()])),
      suppress_on_chapter_start: true,
      suppress_header_on_pages: true,
      suppress_footer_on_pages: true,
      header_separator: false,
      footer_separator: false,
      separator_thickness_pt: 0.5,
      margin_left_in: 0,
      margin_right_in: 0,
      extend_no_header: false,
      extend_no_footer: false,
    };
  }

  // Parse header_footer_json
  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.header_footer_json || '{}');
      const base = defaultSettings();
      if (parsed.slots) {
        for (const k of SLOTS) {
          if (parsed.slots[k]) {
            base.slots[k] = { ...defaultSlot(), ...parsed.slots[k] };
          }
        }
      }
      base.suppress_on_chapter_start = parsed.suppress_on_chapter_start ?? true;
      base.suppress_header_on_pages = parsed.suppress_header_on_pages ?? true;
      base.suppress_footer_on_pages = parsed.suppress_footer_on_pages ?? true;
      base.header_separator = parsed.header_separator ?? false;
      base.footer_separator = parsed.footer_separator ?? false;
      base.separator_thickness_pt = parsed.separator_thickness_pt ?? 0.5;
      base.margin_left_in = parsed.margin_left_in ?? 0;
      base.margin_right_in = parsed.margin_right_in ?? 0;
      base.extend_no_header = parsed.extend_no_header ?? false;
      base.extend_no_footer = parsed.extend_no_footer ?? false;
      return base;
    } catch {
      return defaultSettings();
    }
  });

  // Local working copies
  let slots = $state(structuredClone(settings.slots));
  let suppressOnChapterStart = $state(settings.suppress_on_chapter_start);
  let suppressHeaderOnPages = $state(settings.suppress_header_on_pages);
  let suppressFooterOnPages = $state(settings.suppress_footer_on_pages);
  let headerSeparator = $state(settings.header_separator);
  let footerSeparator = $state(settings.footer_separator);
  let separatorThickness = $state(settings.separator_thickness_pt);
  let marginLeftIn = $state(settings.margin_left_in);
  let marginRightIn = $state(settings.margin_right_in);
  let extendNoHeader = $state(settings.extend_no_header);
  let extendNoFooter = $state(settings.extend_no_footer);

  $effect(() => {
    slots = structuredClone(settings.slots);
    suppressOnChapterStart = settings.suppress_on_chapter_start;
    suppressHeaderOnPages = settings.suppress_header_on_pages;
    suppressFooterOnPages = settings.suppress_footer_on_pages;
    headerSeparator = settings.header_separator;
    footerSeparator = settings.footer_separator;
    separatorThickness = settings.separator_thickness_pt;
    marginLeftIn = settings.margin_left_in;
    marginRightIn = settings.margin_right_in;
    extendNoHeader = settings.extend_no_header;
    extendNoFooter = settings.extend_no_footer;
  });

  // Currently selected slot for editing
  let activeSlotKey = $state(null);
  let activeSlot = $derived(activeSlotKey ? slots[activeSlotKey] : null);

  function selectSlot(key) {
    activeSlotKey = activeSlotKey === key ? null : key;
  }

  function slotLabel(slot) {
    if (!slot || slot.content === 'none') return '';
    const ct = CONTENT_TYPES.find(t => t.id === slot.content);
    if (slot.content === 'custom') return slot.custom || 'Custom';
    return ct?.label || '';
  }

  // Save
  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      slots,
      suppress_on_chapter_start: suppressOnChapterStart,
      suppress_header_on_pages: suppressHeaderOnPages,
      suppress_footer_on_pages: suppressFooterOnPages,
      header_separator: headerSeparator,
      footer_separator: footerSeparator,
      separator_thickness_pt: separatorThickness,
      margin_left_in: marginLeftIn,
      margin_right_in: marginRightIn,
      extend_no_header: extendNoHeader,
      extend_no_footer: extendNoFooter,
    });
    await updateProfileCategory(profile.id, 'header_footer_json', json);
    onchange?.();
  }

  function setSlotContent(contentId) {
    if (!activeSlotKey) return;
    slots[activeSlotKey].content = contentId;
    slots = { ...slots }; // trigger reactivity
    scheduleSave();
  }

  function setSlotFont(font) {
    if (!activeSlotKey) return;
    slots[activeSlotKey].font = font;
    slots = { ...slots };
    scheduleSave();
  }

  function setSlotSize(e) {
    if (!activeSlotKey) return;
    slots[activeSlotKey].size_pt = Number(e.target.value);
    slots = { ...slots };
    scheduleSave();
  }

  function setSlotStyle(styleId) {
    if (!activeSlotKey) return;
    slots[activeSlotKey].style = styleId;
    slots = { ...slots };
    scheduleSave();
  }

  function setSlotCustom(e) {
    if (!activeSlotKey) return;
    slots[activeSlotKey].custom = e.target.value;
    slots = { ...slots };
    scheduleSave();
  }

  function toggleSuppressChapter() {
    suppressOnChapterStart = !suppressOnChapterStart;
    scheduleSave();
  }
  function toggleSuppressHeaderOnPages() {
    suppressHeaderOnPages = !suppressHeaderOnPages;
    scheduleSave();
  }
  function toggleSuppressFooterOnPages() {
    suppressFooterOnPages = !suppressFooterOnPages;
    scheduleSave();
  }
  function toggleExtendNoHeader() {
    extendNoHeader = !extendNoHeader;
    scheduleSave();
  }
  function toggleExtendNoFooter() {
    extendNoFooter = !extendNoFooter;
    scheduleSave();
  }
  function toggleHeaderSep() {
    headerSeparator = !headerSeparator;
    scheduleSave();
  }
  function toggleFooterSep() {
    footerSeparator = !footerSeparator;
    scheduleSave();
  }
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Header / Footer</h4>

  <!-- Visual page diagram -->
  <div class="page-diagram-row">
    <!-- Verso -->
    <div class="page-mini">
      <div class="page-mini-label">Verso (left)</div>
      <div class="page-mini-box">
        <div class="slot-row header-row">
          {#each ['verso_header_left', 'verso_header_center', 'verso_header_right'] as key}
            <button class="slot-cell" class:active={activeSlotKey === key}
              class:filled={slots[key]?.content !== 'none'}
              onclick={() => selectSlot(key)}>
              <span class="slot-text">{slotLabel(slots[key]) || '·'}</span>
            </button>
          {/each}
        </div>
        {#if headerSeparator}<div class="sep-line"></div>{/if}
        <div class="page-body-zone"></div>
        {#if footerSeparator}<div class="sep-line"></div>{/if}
        <div class="slot-row footer-row">
          {#each ['verso_footer_left', 'verso_footer_center', 'verso_footer_right'] as key}
            <button class="slot-cell" class:active={activeSlotKey === key}
              class:filled={slots[key]?.content !== 'none'}
              onclick={() => selectSlot(key)}>
              <span class="slot-text">{slotLabel(slots[key]) || '·'}</span>
            </button>
          {/each}
        </div>
      </div>
    </div>

    <!-- Recto -->
    <div class="page-mini">
      <div class="page-mini-label">Recto (right)</div>
      <div class="page-mini-box">
        <div class="slot-row header-row">
          {#each ['recto_header_left', 'recto_header_center', 'recto_header_right'] as key}
            <button class="slot-cell" class:active={activeSlotKey === key}
              class:filled={slots[key]?.content !== 'none'}
              onclick={() => selectSlot(key)}>
              <span class="slot-text">{slotLabel(slots[key]) || '·'}</span>
            </button>
          {/each}
        </div>
        {#if headerSeparator}<div class="sep-line"></div>{/if}
        <div class="page-body-zone"></div>
        {#if footerSeparator}<div class="sep-line"></div>{/if}
        <div class="slot-row footer-row">
          {#each ['recto_footer_left', 'recto_footer_center', 'recto_footer_right'] as key}
            <button class="slot-cell" class:active={activeSlotKey === key}
              class:filled={slots[key]?.content !== 'none'}
              onclick={() => selectSlot(key)}>
              <span class="slot-text">{slotLabel(slots[key]) || '·'}</span>
            </button>
          {/each}
        </div>
      </div>
    </div>
  </div>

  <!-- Slot editor (shown when a slot is selected) -->
  {#if activeSlotKey && activeSlot}
    <div class="slot-editor">
      <div class="slot-editor-header">
        <span class="slot-editor-title">{activeSlotKey.replace(/_/g, ' ')}</span>
        <button class="slot-editor-close" onclick={() => activeSlotKey = null}>
          <i class="bi bi-x"></i>
        </button>
      </div>

      <div class="slot-field">
        <label class="slot-label">Content</label>
        <div class="content-options">
          {#each CONTENT_TYPES as ct}
            <button class="content-btn" class:selected={activeSlot.content === ct.id}
              onclick={() => setSlotContent(ct.id)}>
              {ct.label}
            </button>
          {/each}
        </div>
      </div>

      {#if activeSlot.content === 'custom'}
        <div class="slot-field">
          <label class="slot-label">Text</label>
          <input type="text" class="slot-input" value={activeSlot.custom}
            oninput={setSlotCustom} placeholder="Enter text" />
        </div>
      {/if}

      {#if activeSlot.content !== 'none'}
        <div class="slot-field">
          <label class="slot-label">Font</label>
          <FontPicker value={activeSlot.font} onchange={setSlotFont}
            placeholder="Default (profile font)" />
        </div>

        <div class="slot-field-row">
          <div class="slot-field" style="flex:1">
            <label class="slot-label">Size</label>
            <select class="slot-select" value={activeSlot.size_pt} onchange={setSlotSize}>
              {#each [7, 8, 9, 10, 11, 12] as s}
                <option value={s}>{s} pt</option>
              {/each}
            </select>
          </div>
          <div class="slot-field" style="flex:1">
            <label class="slot-label">Style</label>
            <select class="slot-select" value={activeSlot.style}
              onchange={(e) => setSlotStyle(e.target.value)}>
              {#each TEXT_STYLES as ts}
                <option value={ts.id}>{ts.label}</option>
              {/each}
            </select>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Global toggles -->
  <div class="setting-group">
    <div class="group-label">Options</div>

    <button class="toggle-row" onclick={toggleSuppressChapter}>
      <span class="toggle-switch" class:on={suppressOnChapterStart}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Suppress on chapter openings</span>
        <span class="toggle-hint">Hide header and footer on the first page of each chapter</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleSuppressHeaderOnPages}>
      <span class="toggle-switch" class:on={suppressHeaderOnPages}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Hide header on front/back matter</span>
        <span class="toggle-hint">No header on custom pages (title, copyright, dedication, etc.)</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleSuppressFooterOnPages}>
      <span class="toggle-switch" class:on={suppressFooterOnPages}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Hide footer on front/back matter</span>
        <span class="toggle-hint">No footer on custom pages — page numbers use roman numerals</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleHeaderSep}>
      <span class="toggle-switch" class:on={headerSeparator}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Header separator line</span>
        <span class="toggle-hint">Thin line between header and body text</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleFooterSep}>
      <span class="toggle-switch" class:on={footerSeparator}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">Footer separator line</span>
        <span class="toggle-hint">Thin line between body text and footer</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleExtendNoHeader}>
      <span class="toggle-switch" class:on={extendNoHeader}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">No header — extend text upward</span>
        <span class="toggle-hint">When no header is set, reduce the top margin so body text fills the space</span>
      </div>
    </button>

    <button class="toggle-row" onclick={toggleExtendNoFooter}>
      <span class="toggle-switch" class:on={extendNoFooter}>
        <span class="toggle-knob"></span>
      </span>
      <div class="toggle-text">
        <span class="toggle-label">No footer — extend text downward</span>
        <span class="toggle-hint">When no footer is set, reduce the bottom margin so body text fills the space</span>
      </div>
    </button>
  </div>

  <div class="setting-group">
    <div class="group-label">Inset</div>
    <div class="inset-hint">Push header/footer content inward from the page edges</div>
    <div class="inset-row">
      <label class="inset-field">
        <span>Left</span>
        <div class="inset-input-wrap">
          <input type="number" {step} min="0"
            value={toDisplay(marginLeftIn)}
            oninput={(e) => { const v = fromDisplay(e.target.value); if (v != null) { marginLeftIn = v; scheduleSave(); } }} />
          <span class="inset-unit">{uLabel}</span>
        </div>
      </label>
      <label class="inset-field">
        <span>Right</span>
        <div class="inset-input-wrap">
          <input type="number" {step} min="0"
            value={toDisplay(marginRightIn)}
            oninput={(e) => { const v = fromDisplay(e.target.value); if (v != null) { marginRightIn = v; scheduleSave(); } }} />
          <span class="inset-unit">{uLabel}</span>
        </div>
      </label>
    </div>
  </div>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }

  /* Page diagram */
  .page-diagram-row {
    display: flex; gap: 0.6rem;
    margin-bottom: 0.8rem;
  }
  .page-mini { flex: 1; }
  .page-mini-label {
    text-align: center;
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .page-mini-box {
    background: #fff;
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.3rem;
    display: flex; flex-direction: column;
    min-height: 140px;
    box-shadow: 0 1px 4px rgba(0,0,0,0.06);
  }
  .slot-row {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 2px;
  }
  .header-row { margin-bottom: 2px; }
  .footer-row { margin-top: 2px; }
  .page-body-zone {
    flex: 1;
    background: repeating-linear-gradient(
      180deg, #eee 0px, #eee 1px, transparent 1px, transparent 8px
    );
    opacity: 0.4;
    min-height: 50px;
    margin: 2px 6px;
    border-radius: 2px;
  }
  .sep-line {
    height: 1px;
    background: var(--iwe-text-muted);
    margin: 0 4px;
    opacity: 0.4;
  }
  .slot-cell {
    padding: 4px 2px;
    border: 1px solid transparent;
    border-radius: 3px;
    background: var(--iwe-bg-warm);
    cursor: pointer;
    transition: all 100ms;
    min-height: 22px;
    display: flex; align-items: center; justify-content: center;
  }
  .slot-cell:hover { border-color: var(--iwe-accent); }
  .slot-cell.active {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.1);
    box-shadow: 0 0 0 1px var(--iwe-accent);
  }
  .slot-cell.filled { background: rgba(45, 106, 94, 0.06); }
  .slot-text {
    font-family: var(--iwe-font-ui); font-size: 0.55rem;
    color: var(--iwe-text-muted);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    max-width: 100%;
  }
  .slot-cell.filled .slot-text { color: var(--iwe-accent); font-weight: 500; }
  .slot-cell.active .slot-text { color: var(--iwe-accent); }

  /* Slot editor */
  .slot-editor {
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.7rem;
    margin-bottom: 0.8rem;
  }
  .slot-editor-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 0.6rem;
  }
  .slot-editor-title {
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    text-transform: capitalize;
    color: var(--iwe-text); font-weight: 500;
  }
  .slot-editor-close {
    border: none; background: none;
    color: var(--iwe-text-muted); cursor: pointer;
    font-size: 1rem; padding: 0;
  }
  .slot-editor-close:hover { color: var(--iwe-text); }

  .slot-field { margin-bottom: 0.6rem; }
  .slot-field-row { display: flex; gap: 0.5rem; margin-bottom: 0.6rem; }
  .slot-label {
    display: block;
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.25rem;
  }
  .slot-input, .slot-select {
    width: 100%;
    padding: 0.35rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .slot-input:focus, .slot-select:focus { outline: none; border-color: var(--iwe-accent); }

  .content-options {
    display: flex; flex-wrap: wrap; gap: 3px;
  }
  .content-btn {
    padding: 3px 8px;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    border: 1px solid var(--iwe-border); border-radius: 8px;
    background: var(--iwe-bg); color: var(--iwe-text);
    cursor: pointer; transition: all 100ms;
  }
  .content-btn:hover { border-color: var(--iwe-accent); }
  .content-btn.selected {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }

  /* Global options */
  .setting-group { margin-bottom: 0.8rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.5rem;
  }
  .toggle-row {
    width: 100%;
    display: flex; align-items: flex-start; gap: 0.7rem;
    padding: 0.45rem 0.1rem;
    background: none; border: none; cursor: pointer;
    text-align: left; transition: background 100ms;
  }
  .toggle-row:hover { background: var(--iwe-bg-hover); }
  .toggle-row + .toggle-row { border-top: 1px solid var(--iwe-border); }
  .toggle-switch {
    flex-shrink: 0; width: 32px; height: 18px;
    border-radius: 9px; background: var(--iwe-border);
    position: relative; transition: background 150ms; margin-top: 2px;
  }
  .toggle-switch.on { background: var(--iwe-accent); }
  .toggle-knob {
    position: absolute; top: 2px; left: 2px;
    width: 14px; height: 14px; border-radius: 50%;
    background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,0.25);
    transition: transform 150ms;
  }
  .toggle-switch.on .toggle-knob { transform: translateX(14px); }
  .toggle-text { display: flex; flex-direction: column; gap: 0.15rem; min-width: 0; }
  .toggle-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .toggle-hint {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); line-height: 1.35;
  }

  /* Inset controls */
  .inset-hint {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); margin-bottom: 0.5rem;
  }
  .inset-row {
    display: grid; grid-template-columns: 1fr 1fr; gap: 0.6rem;
  }
  .inset-field {
    display: flex; flex-direction: column; gap: 0.25rem;
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
  }
  .inset-input-wrap {
    display: flex; align-items: center;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); overflow: hidden;
  }
  .inset-input-wrap:focus-within { border-color: var(--iwe-accent); }
  .inset-input-wrap input {
    flex: 1; min-width: 0; border: none; background: none;
    padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); outline: none;
  }
  .inset-input-wrap input::-webkit-outer-spin-button,
  .inset-input-wrap input::-webkit-inner-spin-button {
    -webkit-appearance: none; margin: 0;
  }
  .inset-unit {
    padding: 0 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border-left: 1px solid var(--iwe-border);
    height: 100%; display: flex; align-items: center;
  }
</style>
