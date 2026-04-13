<script>
  import { onMount } from 'svelte';
  import { updateProfileCategory } from '$lib/db.js';
  import FontPicker from '$lib/components/FontPicker.svelte';
  import DecimalInput from '$lib/components/DecimalInput.svelte';

  let { profile, onchange } = $props();
  let isPrint = $derived(profile?.target_type !== 'ebook');

  function defaults() {
    return {
      // Drop caps
      drop_cap_enabled: false,
      drop_cap_lines: 2,
      drop_cap_font: '',
      drop_cap_color: '#000000',
      drop_cap_quote_mode: 'letter_only', // 'both_together' | 'letter_only' | 'disable_on_dialogue'
      drop_cap_fill_pct: 100,

      // Small caps lead-in
      small_caps_enabled: false,
      small_caps_words: 5,

      // When to apply first-sentence styling
      apply_when: 'chapter', // 'chapter' | 'breaks' | 'both'

      // Subsequent paragraphs
      paragraph_style: 'indented', // 'indented' | 'spaced' | 'both'
      indent_em: 1.5,
      spacing_em: 0.5,

      // Widow/orphan
      prevent_widows: true,
      prevent_orphans: true,

      // Hyphenation
      max_consecutive_hyphens: 3,
      last_line_min_chars: 5,

      // Hyphenation aggressiveness (maps to Typst costs)
      hyphen_aggressiveness: 'normal', // 'low' | 'normal' | 'high'
    };
  }

  let settings = $derived.by(() => {
    try {
      const parsed = JSON.parse(profile?.paragraph_json || '{}');
      return { ...defaults(), ...parsed };
    } catch {
      return defaults();
    }
  });

  // Local state
  let dropCapEnabled = $state(false);
  let dropLines = $state(2);
  let dropCapFont = $state('');
  let dropCapColor = $state('#000000');
  let dropCapQuoteMode = $state('letter_only');
  let dropCapFillPct = $state(0.15);
  let smallCapsEnabled = $state(false);
  let smallCapsWords = $state(5);
  let applyWhen = $state('chapter');
  let paragraphStyle = $state('indented');
  let indentEm = $state(1.5);
  let spacingEm = $state(0.5);
  let preventWidows = $state(2);
  let preventOrphans = $state(2);
  let maxConsecutiveHyphens = $state(3);
  let lastLineMinChars = $state(5);
  let hyphenAggressiveness = $state('normal');

  $effect(() => {
    dropCapEnabled = settings.drop_cap_enabled;
    dropLines = settings.drop_cap_lines;
    dropCapFont = settings.drop_cap_font;
    dropCapColor = settings.drop_cap_color;
    dropCapQuoteMode = settings.drop_cap_quote_mode;
    dropCapFillPct = settings.drop_cap_fill_pct;
    smallCapsEnabled = settings.small_caps_enabled;
    smallCapsWords = settings.small_caps_words;
    applyWhen = settings.apply_when;
    paragraphStyle = settings.paragraph_style;
    indentEm = settings.indent_em;
    spacingEm = settings.spacing_em;
    preventWidows = settings.prevent_widows;
    preventOrphans = settings.prevent_orphans;
    maxConsecutiveHyphens = settings.max_consecutive_hyphens;
    lastLineMinChars = settings.last_line_min_chars;
    hyphenAggressiveness = settings.hyphen_aggressiveness;
  });


  let saveTimer = null;
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  async function persist() {
    if (!profile) return;
    const json = JSON.stringify({
      drop_cap_enabled: dropCapEnabled,
      drop_cap_lines: dropLines,
      drop_cap_font: dropCapFont,
      drop_cap_color: dropCapColor,
      drop_cap_quote_mode: dropCapQuoteMode,
      drop_cap_fill_pct: dropCapFillPct,
      small_caps_enabled: smallCapsEnabled,
      small_caps_words: smallCapsWords,
      apply_when: applyWhen,
      paragraph_style: paragraphStyle,
      indent_em: indentEm,
      spacing_em: spacingEm,
      prevent_widows: preventWidows,
      prevent_orphans: preventOrphans,
      max_consecutive_hyphens: maxConsecutiveHyphens,
      last_line_min_chars: lastLineMinChars,
      hyphen_aggressiveness: hyphenAggressiveness,
    });
    await updateProfileCategory(profile.id, 'paragraph_json', json);
    onchange?.();
  }

  function set(field, value) {
    eval(`${field} = value`); // would be cleaner with a map, but this is concise
    scheduleSave();
  }
</script>

<div class="custom-section">
  <h4 class="section-title">Paragraph</h4>

  <!-- ============ DROP CAPS ============ -->
  <div class="setting-group">
    <div class="group-label">Drop Caps</div>

    <button class="toggle-row" onclick={() => { dropCapEnabled = !dropCapEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={dropCapEnabled}><span class="toggle-knob"></span></span>
      <div class="toggle-text">
        <span class="toggle-label">Enable drop caps</span>
        <span class="toggle-hint">Enlarge the first letter of an opening paragraph</span>
      </div>
    </button>

    {#if dropCapEnabled}
      <div class="sub-settings">
        <!-- Size -->
        <label class="field">
          <span class="field-label">Drop cap height (lines)</span>
          <select class="field-select" bind:value={dropLines} onchange={scheduleSave}>
            {#each [2,3,4,5] as n}<option value={n}>{n} lines</option>{/each}
          </select>
        </label>

        <!-- Font -->
        <div class="field">
          <span class="field-label">Drop cap font</span>
          <FontPicker bind:value={dropCapFont} onchange={(f) => { dropCapFont = f; scheduleSave(); }}
            placeholder="Same as body font" />
        </div>

        <!-- Color -->
        <div class="field">
          <span class="field-label">Drop cap color</span>
          <div class="color-row">
            <input type="color" class="color-input" bind:value={dropCapColor} onchange={scheduleSave} />
            <input type="text" class="color-text" bind:value={dropCapColor} oninput={scheduleSave}
              placeholder="#000000" />
          </div>
        </div>


        <!-- Quote handling -->
        <div class="field">
          <span class="field-label">When text starts with a quote mark</span>
          <div class="radio-group">
            <label class="radio-option" class:selected={dropCapQuoteMode === 'first_char'}>
              <input type="radio" bind:group={dropCapQuoteMode} value="first_char" onchange={scheduleSave} />
              <div>
                <span class="radio-label">First character only</span>
                <span class="radio-hint">Always the very first character — even if it's a quote mark</span>
              </div>
            </label>
            <label class="radio-option" class:selected={dropCapQuoteMode === 'both_together'}>
              <input type="radio" bind:group={dropCapQuoteMode} value="both_together" onchange={scheduleSave} />
              <div>
                <span class="radio-label">Include through first letter</span>
                <span class="radio-hint">Quote mark + first letter enlarged together as a unit</span>
              </div>
            </label>
            <label class="radio-option" class:selected={dropCapQuoteMode === 'letter_only'}>
              <input type="radio" bind:group={dropCapQuoteMode} value="letter_only" onchange={scheduleSave} />
              <div>
                <span class="radio-label">First letter only</span>
                <span class="radio-hint">Skip punctuation — only the first letter drops, quote stays in body text</span>
              </div>
            </label>
            <label class="radio-option" class:selected={dropCapQuoteMode === 'disable_on_dialogue'}>
              <input type="radio" bind:group={dropCapQuoteMode} value="disable_on_dialogue" onchange={scheduleSave} />
              <div>
                <span class="radio-label">No drop cap on dialogue</span>
                <span class="radio-hint">Skip the drop cap entirely when a paragraph opens with a quote</span>
              </div>
            </label>
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- ============ SMALL CAPS LEAD-IN ============ -->
  <div class="setting-group">
    <div class="group-label">Small Caps Lead-in</div>

    <button class="toggle-row" onclick={() => { smallCapsEnabled = !smallCapsEnabled; scheduleSave(); }}>
      <span class="toggle-switch" class:on={smallCapsEnabled}><span class="toggle-knob"></span></span>
      <div class="toggle-text">
        <span class="toggle-label">Enable small caps lead-in</span>
        <span class="toggle-hint">First few words of an opening paragraph in small caps</span>
      </div>
    </button>

    {#if smallCapsEnabled}
      <div class="sub-settings">
        <label class="field">
          <span class="field-label">Number of words</span>
          <select class="field-select" bind:value={smallCapsWords} onchange={scheduleSave}>
            <option value={3}>First 3 words</option>
            <option value={5}>First 5 words</option>
            <option value={8}>First 8 words</option>
            <option value={-1}>To end of first line</option>
          </select>
        </label>
      </div>
    {/if}
  </div>

  <!-- ============ WHEN TO APPLY ============ -->
  <div class="setting-group">
    <div class="group-label">When to Apply First-Sentence Styling</div>
    <div class="radio-group compact">
      <label class="radio-option" class:selected={applyWhen === 'chapter'}>
        <input type="radio" bind:group={applyWhen} value="chapter" onchange={scheduleSave} />
        <span class="radio-label">Beginning of each chapter</span>
      </label>
      <label class="radio-option" class:selected={applyWhen === 'breaks'}>
        <input type="radio" bind:group={applyWhen} value="breaks" onchange={scheduleSave} />
        <span class="radio-label">After every scene break</span>
      </label>
      <label class="radio-option" class:selected={applyWhen === 'both'}>
        <input type="radio" bind:group={applyWhen} value="both" onchange={scheduleSave} />
        <span class="radio-label">Both — chapters and scene breaks</span>
      </label>
    </div>
  </div>

  <!-- ============ SUBSEQUENT PARAGRAPHS ============ -->
  <div class="setting-group">
    <div class="group-label">Subsequent Paragraphs</div>

    <div class="radio-group compact">
      <label class="radio-option" class:selected={paragraphStyle === 'indented'}>
        <input type="radio" bind:group={paragraphStyle} value="indented" onchange={scheduleSave} />
        <span class="radio-label">Indented first line</span>
      </label>
      <label class="radio-option" class:selected={paragraphStyle === 'spaced'}>
        <input type="radio" bind:group={paragraphStyle} value="spaced" onchange={scheduleSave} />
        <span class="radio-label">Block spacing (no indent)</span>
      </label>
      <label class="radio-option" class:selected={paragraphStyle === 'both'}>
        <input type="radio" bind:group={paragraphStyle} value="both" onchange={scheduleSave} />
        <span class="radio-label">Both indent and spacing</span>
      </label>
    </div>

    <div class="sub-settings">
      {#if paragraphStyle === 'indented' || paragraphStyle === 'both'}
        <label class="field">
          <span class="field-label">Indent amount</span>
          <DecimalInput value={indentEm} onchange={(v) => { indentEm = v; scheduleSave(); }}
            suffix="em" step={0.1} min={0} max={5} decimals={1} />
        </label>
      {/if}
      {#if paragraphStyle === 'spaced' || paragraphStyle === 'both'}
        <label class="field">
          <span class="field-label">Paragraph spacing</span>
          <DecimalInput value={spacingEm} onchange={(v) => { spacingEm = v; scheduleSave(); }}
            suffix="em" step={0.1} min={0} max={3} decimals={1} />
        </label>
      {/if}
    </div>
  </div>

  {#if isPrint}
  <!-- ============ WIDOW / ORPHAN ============ -->
  <div class="setting-group">
    <div class="group-label">Widow & Orphan Control</div>

    <button class="toggle-row" onclick={() => { preventWidows = !preventWidows; scheduleSave(); }}>
      <span class="toggle-switch" class:on={preventWidows}><span class="toggle-knob"></span></span>
      <div class="toggle-text">
        <span class="toggle-label">Prevent widows</span>
        <span class="toggle-hint">Avoid a lone line from a paragraph sitting at the top of a page</span>
      </div>
    </button>

    <button class="toggle-row" onclick={() => { preventOrphans = !preventOrphans; scheduleSave(); }}>
      <span class="toggle-switch" class:on={preventOrphans}><span class="toggle-knob"></span></span>
      <div class="toggle-text">
        <span class="toggle-label">Prevent orphans</span>
        <span class="toggle-hint">Avoid a lone opening line of a paragraph at the bottom of a page</span>
      </div>
    </button>
  </div>

  <!-- ============ HYPHENATION ============ -->
  <div class="setting-group">
    <div class="group-label">Hyphenation</div>

    <label class="field">
      <span class="field-label">Max consecutive hyphenated lines</span>
      <span class="field-hint">Limits how many lines in a row can end with a hyphen</span>
      <select class="field-select" bind:value={maxConsecutiveHyphens} onchange={scheduleSave}>
        <option value={2}>2 (strict)</option>
        <option value={3}>3 (standard)</option>
        <option value={4}>4 (relaxed)</option>
        <option value={99}>Unlimited</option>
      </select>
    </label>

    <label class="field">
      <span class="field-label">Last line minimum characters</span>
      <span class="field-hint">Prevents tiny orphaned words at the end of a paragraph</span>
      <select class="field-select" bind:value={lastLineMinChars} onchange={scheduleSave}>
        <option value={0}>No minimum</option>
        <option value={3}>3 characters</option>
        <option value={5}>5 characters</option>
        <option value={8}>8 characters</option>
      </select>
    </label>

    <label class="field">
      <span class="field-label">Hyphenation aggressiveness</span>
      <span class="field-hint">How eagerly to break words to improve line spacing</span>
      <div class="radio-group compact">
        <label class="radio-option" class:selected={hyphenAggressiveness === 'low'}>
          <input type="radio" bind:group={hyphenAggressiveness} value="low" onchange={scheduleSave} />
          <span class="radio-label">Low — fewer hyphens, more uneven spacing</span>
        </label>
        <label class="radio-option" class:selected={hyphenAggressiveness === 'normal'}>
          <input type="radio" bind:group={hyphenAggressiveness} value="normal" onchange={scheduleSave} />
          <span class="radio-label">Normal — balanced</span>
        </label>
        <label class="radio-option" class:selected={hyphenAggressiveness === 'high'}>
          <input type="radio" bind:group={hyphenAggressiveness} value="high" onchange={scheduleSave} />
          <span class="radio-label">High — more hyphens, tighter justification</span>
        </label>
      </div>
    </label>
  </div>
  {/if}
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.8rem 0; color: var(--iwe-text);
  }

  .setting-group { margin-bottom: 1rem; }
  .group-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.4rem;
  }
  .sub-settings {
    padding: 0.5rem 0 0.2rem 0;
    display: flex; flex-direction: column; gap: 0.7rem;
  }

  /* Fields */
  .field { display: flex; flex-direction: column; gap: 0.2rem; }
  .field-label {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
  }
  .field-hint {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-muted); font-style: italic;
    font-weight: 400; text-transform: none; letter-spacing: 0;
  }
  .field-select {
    padding: 0.4rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
  }
  .field-select:focus { outline: none; border-color: var(--iwe-accent); }
  .field-num {
    flex: 1; min-width: 0; padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .field-num:focus { outline: none; border-color: var(--iwe-accent); }
  .field-num::-webkit-outer-spin-button,
  .field-num::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .row-2col { display: grid; grid-template-columns: 1fr 1fr; gap: 0.6rem; }
  .input-row { display: flex; align-items: center; gap: 0.4rem; }
  .unit {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); white-space: nowrap;
  }

  /* Color picker */
  .color-row { display: flex; align-items: center; gap: 0.5rem; }
  .color-input {
    width: 36px; height: 30px;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    padding: 2px; cursor: pointer; background: var(--iwe-bg);
  }
  .color-text {
    flex: 1; padding: 0.35rem 0.5rem;
    font-family: monospace; font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .color-text:focus { outline: none; border-color: var(--iwe-accent); }
  .fill-slider { width: 100%; accent-color: var(--iwe-accent); }

  /* Size preview */
  .size-preview {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); padding: 0.4rem 0.6rem;
    background: var(--iwe-bg-warm); border-radius: var(--iwe-radius-sm);
    border-left: 2px solid var(--iwe-accent);
  }

  /* Radio options */
  .radio-group { display: flex; flex-direction: column; gap: 3px; }
  .radio-group.compact { gap: 2px; }
  .radio-option {
    display: flex; align-items: flex-start; gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); cursor: pointer;
    transition: all 100ms;
  }
  .radio-option:hover { border-color: var(--iwe-accent); }
  .radio-option.selected {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
  }
  .radio-option input[type="radio"] {
    margin-top: 3px; accent-color: var(--iwe-accent);
  }
  .radio-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text);
  }
  .radio-hint {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); display: block; margin-top: 0.1rem;
  }

  /* Toggle */
  .toggle-row {
    width: 100%;
    display: flex; align-items: flex-start; gap: 0.7rem;
    padding: 0.45rem 0.1rem;
    background: none; border: none; cursor: pointer;
    text-align: left; transition: background 100ms;
  }
  .toggle-row:hover { background: var(--iwe-bg-hover); }
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
  .toggle-text { display: flex; flex-direction: column; gap: 0.1rem; min-width: 0; }
  .toggle-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text); font-weight: 500;
  }
  .toggle-hint {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); line-height: 1.35;
  }
</style>
