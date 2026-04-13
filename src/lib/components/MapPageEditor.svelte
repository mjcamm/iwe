<script>
  import { onMount } from 'svelte';
  import { ensureUnitLoaded, toDisplay, fromDisplay, unitLabel, unitStep, subscribe } from '$lib/unitPreference.js';
  import DecimalInput from '$lib/components/DecimalInput.svelte';

  let { page, profile, onsave, oncancel } = $props();

  let unit = $state('in');
  let unitLoaded = $state(false);

  onMount(async () => {
    unit = await ensureUnitLoaded();
    unitLoaded = true;
    return subscribe(u => { unit = u; });
  });

  // Resolution check: 300 DPI is the print standard.
  const PRINT_DPI = 300;
  let imageNaturalWidth = $state(0);
  let imageNaturalHeight = $state(0);

  // Required pixel dimensions based on trim size and spread
  let requiredWidth = $derived(Math.round((profile?.trim_width_in ?? 6) * (spread ? 2 : 1) * PRINT_DPI));
  let requiredHeight = $derived(Math.round((profile?.trim_height_in ?? 9) * PRINT_DPI));

  let resolutionWarning = $derived(() => {
    if (!imageData || !imageNaturalWidth || !imageNaturalHeight) return null;
    const wOk = imageNaturalWidth >= requiredWidth;
    const hOk = imageNaturalHeight >= requiredHeight;
    if (wOk && hOk) return null;
    const actualDpiW = Math.round(imageNaturalWidth / ((profile?.trim_width_in ?? 6) * (spread ? 2 : 1)));
    const actualDpiH = Math.round(imageNaturalHeight / (profile?.trim_height_in ?? 9));
    const effectiveDpi = Math.min(actualDpiW, actualDpiH);
    return `Image is ${imageNaturalWidth} x ${imageNaturalHeight}px (${effectiveDpi} DPI). For print quality at this trim size, ${requiredWidth} x ${requiredHeight}px (300 DPI) is recommended.`;
  });

  function measureImage(src) {
    if (!src) { imageNaturalWidth = 0; imageNaturalHeight = 0; return; }
    const img = new Image();
    img.onload = () => {
      imageNaturalWidth = img.naturalWidth;
      imageNaturalHeight = img.naturalHeight;
    };
    img.onerror = () => { imageNaturalWidth = 0; imageNaturalHeight = 0; };
    img.src = src;
  }

  // Measure whenever imageData changes
  $effect(() => { measureImage(imageData); });

  function parseSettings(raw) {
    if (raw && raw.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(raw);
        return {
          spread: parsed.spread ?? false,
          image_data: parsed.image_data ?? '',
          sizing: parsed.sizing ?? 'fit-width',
          gutter_overlap_in: parsed.gutter_overlap_in ?? 0.25,
        };
      } catch { /* fall through */ }
    }
    return { spread: false, image_data: '', sizing: 'fit-width', gutter_overlap_in: 0.25 };
  }

  const initial = parseSettings(page.content);
  let spread = $state(initial.spread);
  let imageData = $state(initial.image_data);
  let sizing = $state(initial.sizing);
  let gutterOverlap = $state(initial.gutter_overlap_in);

  let fileInput;

  function handleFileSelected(e) {
    const file = e.target.files?.[0];
    if (!file || !file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = () => { imageData = reader.result; };
    reader.readAsDataURL(file);
    e.target.value = '';
  }

  function clearImage() {
    imageData = '';
  }

  function handleSave() {
    const content = JSON.stringify({
      spread,
      image_data: imageData,
      sizing,
      gutter_overlap_in: spread ? gutterOverlap : 0,
    });
    onsave({ content });
  }

  function handleCancel() {
    oncancel();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      handleCancel();
    } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="map-editor-backdrop" onclick={handleCancel}>
  <div class="map-editor-shell" onclick={(e) => e.stopPropagation()}>
    <div class="editor-header">
      <div class="editor-title">
        <strong>Map Page</strong>
        <span class="editor-role-badge">map</span>
      </div>
      <div class="editor-header-actions">
        <button class="editor-btn" onclick={handleCancel}>Cancel</button>
        <button class="editor-btn primary" onclick={handleSave}>Save (Ctrl+S)</button>
      </div>
    </div>

    <div class="editor-body">
      <!-- Spread toggle -->
      <div class="setting-group">
        <label class="setting-label">Page layout</label>
        <div class="layout-options">
          <button class="layout-option" class:active={!spread}
            onclick={() => spread = false}>
            <div class="layout-icon single-icon"></div>
            <span>Single page</span>
          </button>
          <button class="layout-option" class:active={spread}
            onclick={() => spread = true}>
            <div class="layout-icon spread-icon">
              <div class="spread-left"></div>
              <div class="spread-right"></div>
            </div>
            <span>Double spread</span>
          </button>
        </div>
        {#if spread}
          <span class="setting-hint">Image will be split across a left–right page pair</span>

          {#if unitLoaded}
            <div class="gutter-overlap-group">
              <label class="setting-label">Gutter overlap</label>
              {#key unit}
                <div class="gutter-input-row">
                  <DecimalInput
                    value={toDisplay(gutterOverlap)}
                    onchange={(v) => { const inches = fromDisplay(v); if (inches != null) gutterOverlap = inches; }}
                    suffix={unitLabel()}
                    step={unitStep()}
                    min={0}
                    max={unit === 'mm' ? 25 : 1} />
                </div>
              {/key}
              <span class="setting-hint">The center of the image can be lost in the book's spine when bound. This duplicates a strip from the middle onto both pages so nothing is missing when the book is opened. Typical: {unit === 'mm' ? '6mm (paperback), 10mm (hardcover)' : '0.25" (paperback), 0.375" (hardcover)'}.</span>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Image upload -->
      <div class="setting-group">
        <label class="setting-label">Image</label>
        {#if imageData}
          <div class="image-preview-wrap">
            <img class="image-preview" src={imageData} alt="Map preview"
              class:spread-preview={spread} />
            <div class="image-actions">
              <button class="editor-btn" onclick={() => fileInput?.click()}>Replace</button>
              <button class="editor-btn danger" onclick={clearImage}>Remove</button>
            </div>
          </div>
        {:else}
          <button class="upload-btn" onclick={() => fileInput?.click()}>
            <i class="bi bi-image"></i>
            <span>Choose image</span>
          </button>
        {/if}
        <input type="file" accept="image/*" bind:this={fileInput}
          onchange={handleFileSelected} style="display: none;" />
        {#if resolutionWarning()}
          <div class="resolution-warning">
            <i class="bi bi-exclamation-triangle"></i>
            <span>{resolutionWarning()}</span>
          </div>
        {/if}
        {#if imageData && imageNaturalWidth && !resolutionWarning()}
          <div class="resolution-ok">
            <i class="bi bi-check-circle"></i>
            <span>{imageNaturalWidth} x {imageNaturalHeight}px — good for print at this size</span>
          </div>
        {/if}
      </div>

      <!-- Sizing mode -->
      <div class="setting-group">
        <label class="setting-label">Sizing</label>
        <div class="sizing-options">
          <button class="sizing-option" class:active={sizing === 'fit-width'}
            onclick={() => sizing = 'fit-width'}>
            <div class="sizing-info">
              <span class="sizing-name">Fit width</span>
              <span class="sizing-desc">Fills page width, clips top/bottom if needed</span>
            </div>
          </button>
          <button class="sizing-option" class:active={sizing === 'fit-height'}
            onclick={() => sizing = 'fit-height'}>
            <div class="sizing-info">
              <span class="sizing-name">Fit height</span>
              <span class="sizing-desc">Fills page height, clips left/right if needed</span>
            </div>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .map-editor-backdrop {
    position: fixed; inset: 0; z-index: 2000;
    background: rgba(0, 0, 0, 0.55);
    display: flex; align-items: center; justify-content: center;
  }
  .map-editor-shell {
    background: var(--iwe-bg);
    width: 520px; max-width: 95vw;
    border-radius: var(--iwe-radius);
    display: flex; flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .editor-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
    flex-shrink: 0;
  }
  .editor-title {
    display: flex; align-items: center; gap: 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    color: var(--iwe-text);
  }
  .editor-role-badge {
    font-size: 0.65rem; text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    background: var(--iwe-bg-hover); color: var(--iwe-text-muted);
    padding: 2px 8px; border-radius: 8px;
  }
  .editor-header-actions { display: flex; gap: 0.5rem; }
  .editor-btn {
    padding: 0.4rem 0.9rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
  }
  .editor-btn:hover { background: var(--iwe-bg-hover); }
  .editor-btn.primary {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .editor-btn.primary:hover { background: #245a4f; border-color: #245a4f; }
  .editor-btn.danger { color: #c0392b; }
  .editor-btn.danger:hover { background: rgba(192, 57, 43, 0.08); }

  .editor-body {
    padding: 1.2rem 1.2rem 1.5rem;
    display: flex; flex-direction: column; gap: 1.2rem;
    overflow-y: auto;
    max-height: 80vh;
  }

  .setting-group {
    display: flex; flex-direction: column; gap: 0.35rem;
  }
  .setting-label {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .setting-hint {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); line-height: 1.35;
  }

  /* Layout toggle */
  .layout-options {
    display: flex; gap: 8px;
  }
  .layout-option {
    flex: 1;
    display: flex; flex-direction: column; align-items: center; gap: 0.5rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.8rem 0.6rem;
    cursor: pointer; transition: all 100ms;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
  }
  .layout-option:hover { border-color: var(--iwe-accent); }
  .layout-option.active {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
    box-shadow: 0 0 0 1px var(--iwe-accent);
  }

  .layout-icon {
    width: 48px; height: 60px;
    border: 2px solid var(--iwe-text-muted);
    border-radius: 3px;
    background: var(--iwe-bg-warm);
  }
  .layout-option.active .layout-icon {
    border-color: var(--iwe-accent);
  }
  .single-icon {
    background: repeating-linear-gradient(
      45deg, transparent, transparent 4px,
      rgba(45, 106, 94, 0.08) 4px, rgba(45, 106, 94, 0.08) 8px
    );
  }
  .spread-icon {
    width: 88px;
    display: flex; gap: 2px;
    background: none; border: none;
  }
  .spread-left, .spread-right {
    flex: 1; height: 60px;
    border: 2px solid var(--iwe-text-muted);
    border-radius: 3px;
    background: repeating-linear-gradient(
      45deg, transparent, transparent 4px,
      rgba(45, 106, 94, 0.08) 4px, rgba(45, 106, 94, 0.08) 8px
    );
  }
  .layout-option.active .spread-left,
  .layout-option.active .spread-right {
    border-color: var(--iwe-accent);
  }

  /* Image upload */
  .upload-btn {
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    border: 2px dashed var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text-muted);
    padding: 1.5rem;
    cursor: pointer; transition: all 100ms;
    font-family: var(--iwe-font-ui); font-size: 0.88rem;
  }
  .upload-btn:hover {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
  }
  .upload-btn i { font-size: 1.3rem; }

  .image-preview-wrap {
    display: flex; flex-direction: column; gap: 0.5rem; align-items: center;
  }
  .image-preview {
    max-width: 100%; max-height: 280px;
    border-radius: var(--iwe-radius-sm);
    border: 1px solid var(--iwe-border);
    object-fit: contain;
  }
  .image-preview.spread-preview {
    max-height: 200px;
  }
  .image-actions {
    display: flex; gap: 0.4rem;
  }

  /* Sizing */
  .sizing-options {
    display: flex; flex-direction: column; gap: 6px;
  }
  .sizing-option {
    display: flex; align-items: center; gap: 0.75rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.55rem 0.75rem;
    cursor: pointer; transition: all 100ms;
    text-align: left;
    font-family: var(--iwe-font-ui);
  }
  .sizing-option:hover { border-color: var(--iwe-accent); }
  .sizing-option.active {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
    box-shadow: 0 0 0 1px var(--iwe-accent);
  }
  .sizing-info {
    display: flex; flex-direction: column; gap: 1px;
  }
  .sizing-name {
    font-size: 0.88rem; font-weight: 600;
  }
  .sizing-desc {
    font-size: 0.78rem; color: var(--iwe-text-muted);
  }

  /* Resolution warnings */
  /* Gutter overlap */
  .gutter-overlap-group {
    margin-top: 0.6rem;
    display: flex; flex-direction: column; gap: 0.35rem;
  }
  .gutter-input-row {
    display: flex; align-items: center; gap: 0.5rem;
    max-width: 140px;
  }

  .resolution-warning {
    display: flex; align-items: flex-start; gap: 0.5rem;
    background: rgba(217, 119, 6, 0.08);
    border: 1px solid rgba(217, 119, 6, 0.3);
    border-radius: var(--iwe-radius-sm);
    padding: 0.5rem 0.7rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: #92400e;
  }
  .resolution-warning i { color: #d97706; margin-top: 1px; flex-shrink: 0; }
  .resolution-ok {
    display: flex; align-items: center; gap: 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-accent);
  }
  .resolution-ok i { flex-shrink: 0; }
</style>
