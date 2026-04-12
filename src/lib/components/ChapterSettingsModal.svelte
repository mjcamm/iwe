<script>
  import { updateChapterMetadata } from '$lib/db.js';

  let { chapter, onsave, oncancel, ondelete } = $props();

  let confirmDelete = $state(false);

  let title = $state(chapter.title || '');
  let subtitle = $state(chapter.subtitle || '');
  let chapterImage = $state(chapter.chapter_image || '');

  let fileInputImage;

  async function handleSave() {
    await updateChapterMetadata(
      chapter.id, title.trim(), subtitle.trim(), chapterImage,
      '', '', '',
    );
    onsave?.();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') oncancel?.();
    if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
  }

  function handleImageUpload(e, setter) {
    const file = e.target.files?.[0];
    if (!file || !file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = () => setter(reader.result);
    reader.readAsDataURL(file);
    e.target.value = '';
  }

  function clearImage(setter) { setter(''); }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="modal-backdrop" onclick={oncancel}>
  <div class="modal-card" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h3>Chapter Settings</h3>
      <button class="modal-close" onclick={oncancel}><i class="bi bi-x-lg"></i></button>
    </div>

    <div class="modal-body">
      <!-- Title -->
      <label class="field">
        <span class="field-label">Title</span>
        <input type="text" class="field-input" bind:value={title} placeholder="Chapter title" />
      </label>

      <!-- Subtitle -->
      <label class="field">
        <span class="field-label">Subtitle</span>
        <input type="text" class="field-input" bind:value={subtitle}
          placeholder="Optional subtitle (appears below the title)" />
      </label>

      <!-- Chapter image -->
      <div class="field">
        <span class="field-label">Chapter Image</span>
        <span class="field-hint">A thematic image for this chapter (position and size are set in formatting settings)</span>
        {#if chapterImage}
          <div class="ornament-preview">
            <img src={chapterImage} alt="Chapter image" style="max-height: 120px;" />
            <div class="ornament-actions">
              <button class="orn-btn" onclick={() => fileInputImage?.click()}>Replace</button>
              <button class="orn-btn danger" onclick={() => clearImage(v => chapterImage = v)}>Remove</button>
            </div>
          </div>
        {:else}
          <button class="upload-btn" onclick={() => fileInputImage?.click()}>
            <i class="bi bi-image"></i> Upload image (PNG/SVG/JPEG)
          </button>
        {/if}
        <input type="file" accept="image/png,image/svg+xml,image/jpeg"
          bind:this={fileInputImage}
          onchange={(e) => handleImageUpload(e, v => chapterImage = v)}
          style="display:none" />
      </div>

    </div>

    <div class="modal-footer">
      <div class="footer-left">
        {#if confirmDelete}
          <span class="delete-confirm-text">Delete this chapter?</span>
          <button class="btn danger" onclick={() => { ondelete?.(chapter.id); }}>Yes, delete</button>
          <button class="btn" onclick={() => confirmDelete = false}>No</button>
        {:else}
          <button class="btn danger-ghost" onclick={() => confirmDelete = true}>
            <i class="bi bi-trash3"></i> Delete chapter
          </button>
        {/if}
      </div>
      <div class="footer-right">
        <button class="btn" onclick={oncancel}>Cancel</button>
        <button class="btn primary" onclick={handleSave}>Save (Ctrl+S)</button>
      </div>
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed; inset: 0; z-index: 2000;
    background: rgba(0,0,0,0.45);
    display: flex; align-items: center; justify-content: center;
  }
  .modal-card {
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius);
    width: 520px; max-width: 95vw; max-height: 90vh;
    display: flex; flex-direction: column;
    box-shadow: 0 20px 60px rgba(0,0,0,0.3);
  }
  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.8rem 1.2rem;
    border-bottom: 1px solid var(--iwe-border);
  }
  .modal-header h3 {
    margin: 0; font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 1.1rem; color: var(--iwe-text);
  }
  .modal-close {
    border: none; background: none; color: var(--iwe-text-muted);
    font-size: 1rem; cursor: pointer; padding: 0.2rem;
  }
  .modal-close:hover { color: var(--iwe-text); }
  .modal-body {
    padding: 1.2rem;
    overflow-y: auto;
    display: flex; flex-direction: column; gap: 1rem;
  }
  .modal-footer {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.8rem 1.2rem;
    border-top: 1px solid var(--iwe-border);
  }
  .footer-left { display: flex; align-items: center; gap: 0.4rem; }
  .footer-right { display: flex; align-items: center; gap: 0.5rem; }
  .delete-confirm-text {
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    color: #c0392b; margin-right: 0.3rem;
  }

  .field { display: flex; flex-direction: column; gap: 0.35rem; }
  .field-label {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .field-input {
    padding: 0.55rem 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 1rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .field-input:focus { outline: none; border-color: var(--iwe-accent); }
  .field-hint {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    color: var(--iwe-text-muted); font-style: italic;
    font-weight: 400; text-transform: none;
  }
  .field-input::placeholder { color: var(--iwe-text-muted); opacity: 0.5; }

  .upload-btn {
    display: flex; align-items: center; justify-content: center; gap: 0.4rem;
    padding: 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border: 1px dashed var(--iwe-border); border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .upload-btn:hover { color: var(--iwe-accent); border-color: var(--iwe-accent); }

  .ornament-preview {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.6rem;
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
  }
  .ornament-preview img {
    display: block; max-width: 100%; max-height: 80px;
    margin: 0 auto;
    background: #fff; padding: 0.4rem;
    border-radius: 3px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }
  .ornament-actions { display: flex; gap: 0.4rem; }
  .orn-btn {
    flex: 1; padding: 0.4rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    color: var(--iwe-text); cursor: pointer; transition: all 100ms;
  }
  .orn-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .orn-btn.danger:hover { border-color: #c0392b; color: #c0392b; }

  .btn {
    padding: 0.5rem 1.1rem;
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    cursor: pointer; transition: all 100ms;
  }
  .btn:hover { background: var(--iwe-bg-hover); }
  .btn.primary {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .btn.primary:hover { background: #245a4f; }
  .btn.danger {
    background: #c0392b; border-color: #c0392b; color: #fff;
  }
  .btn.danger:hover { background: #a93226; }
  .btn.danger-ghost {
    background: none; border-color: transparent; color: var(--iwe-text-muted);
  }
  .btn.danger-ghost:hover { color: #c0392b; border-color: #c0392b; }
</style>
