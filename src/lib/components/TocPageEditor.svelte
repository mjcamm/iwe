<script>
  import FontPicker from '$lib/components/FontPicker.svelte';

  let { page, profile, onsave, oncancel } = $props();

  // Parse existing TOC settings from the page content (JSON), or use defaults.
  function parseSettings(raw) {
    if (raw && raw.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(raw);
        return {
          toc_title: parsed.toc_title ?? 'Contents',
          leader_style: parsed.leader_style ?? 'dots',
          title_font: parsed.title_font ?? '',
          item_spacing_em: parsed.item_spacing_em ?? 0.5,
          vertical_align: parsed.vertical_align ?? 'top',
        };
      } catch { /* fall through */ }
    }
    return { toc_title: 'Contents', leader_style: 'dots', title_font: '', item_spacing_em: 0.5, vertical_align: 'top' };
  }

  const initial = parseSettings(page.content);
  let tocTitle = $state(initial.toc_title);
  let leaderStyle = $state(initial.leader_style);
  let titleFont = $state(initial.title_font);
  let itemSpacing = $state(initial.item_spacing_em);
  let verticalAlign = $state(initial.vertical_align);

  // Body font from the profile's typography settings (read-only, shown for reference)
  let bodyFont = $derived(() => {
    try {
      if (profile?.typography_json) {
        const t = JSON.parse(profile.typography_json);
        if (t.font) return t.font;
      }
    } catch { /* ignore */ }
    return profile?.font_body || 'Liberation Serif';
  });

  function handleSave() {
    const content = JSON.stringify({
      toc_title: tocTitle.trim() || 'Contents',
      leader_style: leaderStyle,
      title_font: titleFont,
      item_spacing_em: itemSpacing,
      vertical_align: verticalAlign,
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

  const LEADER_OPTIONS = [
    { value: 'dots',   label: 'Dots',   preview: 'Chapter One . . . . . . . . 1' },
    { value: 'dashes', label: 'Dashes', preview: 'Chapter One \u2013 \u2013 \u2013 \u2013 \u2013 1' },
    { value: 'none',   label: 'None',   preview: 'Chapter One                        1' },
  ];

  const SPACING_OPTIONS = [
    { value: 0.15, label: 'Tight' },
    { value: 0.4,  label: 'Normal' },
    { value: 0.8,  label: 'Relaxed' },
    { value: 1.4,  label: 'Loose' },
    { value: 2.25, label: 'Wide' },
  ];
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="toc-editor-backdrop" onclick={handleCancel}>
  <div class="toc-editor-shell" onclick={(e) => e.stopPropagation()}>
    <div class="editor-header">
      <div class="editor-title">
        <strong>Table of Contents</strong>
        <span class="editor-role-badge">toc</span>
      </div>
      <div class="editor-header-actions">
        <button class="editor-btn" onclick={handleCancel}>Cancel</button>
        <button class="editor-btn primary" onclick={handleSave}>Save (Ctrl+S)</button>
      </div>
    </div>

    <div class="editor-body">
      <div class="setting-group">
        <label class="setting-label" for="toc-title">Title shown on page</label>
        <input id="toc-title" class="setting-input" type="text"
          bind:value={tocTitle}
          placeholder="Contents" />
      </div>

      <div class="setting-group">
        <label class="setting-label">Title font</label>
        <div class="font-picker-wrap">
          <FontPicker
            value={titleFont}
            onchange={(font) => titleFont = font || ''}
            placeholder="Profile body font" />
        </div>
        <span class="setting-hint">Leave blank to use the profile's body font ({bodyFont()})</span>
      </div>

      <div class="setting-group">
        <label class="setting-label">Leader style</label>
        <div class="leader-options">
          {#each LEADER_OPTIONS as opt}
            <button class="leader-option" class:active={leaderStyle === opt.value}
              onclick={() => leaderStyle = opt.value}>
              <span class="leader-label">{opt.label}</span>
              <span class="leader-preview">{opt.preview}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="setting-group">
        <label class="setting-label">Item spacing</label>
        <div class="spacing-options">
          {#each SPACING_OPTIONS as opt}
            <button class="spacing-option" class:active={itemSpacing === opt.value}
              onclick={() => itemSpacing = opt.value}>
              {opt.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="setting-group">
        <label class="setting-label">Page alignment</label>
        <div class="align-options">
          <button class="align-option" class:active={verticalAlign === 'top'}
            onclick={() => verticalAlign = 'top'}>
            <i class="bi bi-align-top"></i> Top
          </button>
          <button class="align-option" class:active={verticalAlign === 'center'}
            onclick={() => verticalAlign = 'center'}>
            <i class="bi bi-align-middle"></i> Center
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .toc-editor-backdrop {
    position: fixed; inset: 0; z-index: 2000;
    background: rgba(0, 0, 0, 0.55);
    display: flex; align-items: center; justify-content: center;
  }
  .toc-editor-shell {
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
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .setting-input {
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    font-family: var(--iwe-font-ui); font-size: 0.92rem;
    padding: 0.45rem 0.6rem;
    transition: border-color 100ms;
  }
  .setting-input:focus { outline: none; border-color: var(--iwe-accent); }
  .setting-hint {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
  }

  .font-picker-wrap {
    max-width: 280px;
  }

  .leader-options {
    display: flex; flex-direction: column; gap: 6px;
  }
  .leader-option {
    display: flex; flex-direction: column; gap: 2px;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.5rem 0.7rem;
    cursor: pointer; transition: all 100ms;
    text-align: left;
  }
  .leader-option:hover { border-color: var(--iwe-accent); }
  .leader-option.active {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06);
    box-shadow: 0 0 0 1px var(--iwe-accent);
  }
  .leader-label {
    font-family: var(--iwe-font-ui); font-size: 0.82rem; font-weight: 600;
    color: var(--iwe-text);
  }
  .leader-preview {
    font-family: 'Liberation Serif', 'Georgia', serif; font-size: 0.78rem;
    color: var(--iwe-text-muted);
    white-space: pre;
    letter-spacing: 0.02em;
  }

  .spacing-options {
    display: flex; gap: 4px; flex-wrap: wrap;
  }
  .spacing-option {
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.35rem 0.7rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    cursor: pointer; transition: all 100ms;
  }
  .spacing-option:hover { border-color: var(--iwe-accent); }
  .spacing-option.active {
    border-color: var(--iwe-accent);
    background: var(--iwe-accent); color: #fff;
  }

  .align-options {
    display: flex; gap: 4px;
  }
  .align-option {
    display: flex; align-items: center; gap: 0.4rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.4rem 0.8rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    cursor: pointer; transition: all 100ms;
  }
  .align-option:hover { border-color: var(--iwe-accent); }
  .align-option.active {
    border-color: var(--iwe-accent);
    background: var(--iwe-accent); color: #fff;
  }
</style>
