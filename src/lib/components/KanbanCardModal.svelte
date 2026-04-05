<script>
  let {
    show = false,
    card = null,
    isNew = false,
    onCreate,
    onSave,
    onDelete,
    onClose,
  } = $props();

  let description = $state('');
  let confirmDelete = $state(false);
  let descInput = $state(null);

  $effect(() => {
    if (show) {
      if (isNew) {
        description = '';
      } else if (card) {
        description = card.description || card.text || '';
      }
      confirmDelete = false;
      setTimeout(() => descInput?.focus(), 50);
    }
  });

  function handleSave() {
    if (!isNew) {
      onSave?.({ ...card, description });
    }
  }

  function handleCreate() {
    if (!description.trim()) return;
    onCreate?.({ description: description.trim() });
  }

  function handleDone() {
    handleSave();
    onClose?.();
  }

  function handleDelete() {
    if (!confirmDelete) { confirmDelete = true; return; }
    onDelete?.(card);
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      if (!isNew) handleSave();
      onClose?.();
    }
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) {
      if (!isNew) handleSave();
      onClose?.();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="km-backdrop" onclick={handleBackdropClick}>
    <div class="km-modal" role="dialog" aria-modal="true">
      <div class="km-header">
        <span class="km-header-title">{isNew ? 'New Card' : 'Edit Card'}</span>
        <button class="km-close" onclick={() => { if (!isNew) handleSave(); onClose?.(); }}>&times;</button>
      </div>

      <div class="km-body">
        <textarea
          bind:this={descInput}
          class="km-desc"
          placeholder="Write your note..."
          bind:value={description}
          onblur={() => { if (!isNew) handleSave(); }}
          rows="8"
        ></textarea>
      </div>

      <div class="km-footer">
        {#if isNew}
          <span></span>
          <div class="km-actions">
            <button class="km-cancel" onclick={() => onClose?.()}>Cancel</button>
            <button class="km-create" onclick={handleCreate} disabled={!description.trim()}>
              <i class="bi bi-plus-lg"></i> Create
            </button>
          </div>
        {:else}
          <span class="km-date">{card?.created_at ? new Date(card.created_at).toLocaleDateString() : ''}</span>
          <div class="km-actions">
            {#if confirmDelete}
              <button class="km-delete-confirm" onclick={handleDelete}>Confirm delete</button>
              <button class="km-cancel" onclick={() => confirmDelete = false}>Cancel</button>
            {:else}
              <button class="km-delete" onclick={handleDelete}>
                <i class="bi bi-trash3"></i> Delete
              </button>
              <button class="km-done" onclick={handleDone}>Done</button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .km-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.35);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 8vh;
    animation: km-fade 0.15s ease;
  }
  @keyframes km-fade { from { opacity: 0; } to { opacity: 1; } }

  .km-modal {
    background: var(--iwe-bg, #fff);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2), 0 4px 16px rgba(0,0,0,0.1);
    width: 90vw; max-width: 560px;
    display: flex; flex-direction: column;
    animation: km-slide 0.2s ease;
  }
  @keyframes km-slide {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .km-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.8rem 1.3rem;
    border-bottom: 1px solid var(--iwe-border-light, #f0ede8);
  }
  .km-header-title {
    font-family: var(--iwe-font-ui, 'Source Sans 3'); font-size: 0.85rem;
    font-weight: 600; color: var(--iwe-text, #2d2a26);
  }
  .km-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.6rem; line-height: 1; padding: 0.2rem 0.4rem;
    color: var(--iwe-text-faint); border-radius: var(--iwe-radius-sm, 4px);
  }
  .km-close:hover { color: var(--iwe-text); background: var(--iwe-bg-hover); }

  .km-body {
    padding: 0.5rem 1.3rem 1rem;
  }
  .km-label {
    font-family: var(--iwe-font-ui, 'Source Sans 3'); font-size: 0.72rem;
    font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
    color: var(--iwe-text-faint, #c8c3bb); margin-bottom: 0.4rem; display: block;
  }
  .km-desc {
    width: 100%; border: 1px solid var(--iwe-border, #e5e1da);
    border-radius: 6px; padding: 0.7rem 0.8rem;
    font-family: var(--iwe-font-prose, 'Libre Baskerville'); font-size: 1rem;
    color: var(--iwe-text, #3d3a37); background: var(--iwe-bg-warm, #faf8f5);
    resize: vertical; outline: none; line-height: 1.7;
  }
  .km-desc:focus { border-color: var(--iwe-accent, #2d6a5e); }
  .km-desc::placeholder { color: var(--iwe-text-faint); }

  .km-footer {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.7rem 1.3rem;
    border-top: 1px solid var(--iwe-border-light, #f0ede8);
  }
  .km-date {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-faint);
  }
  .km-actions { display: flex; gap: 0.4rem; }
  .km-done {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.3rem 0.8rem; border: 1px solid var(--iwe-accent, #2d6a5e);
    border-radius: var(--iwe-radius-sm, 4px); cursor: pointer;
    background: var(--iwe-accent, #2d6a5e); color: white;
  }
  .km-done:hover { opacity: 0.9; }
  .km-create {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.3rem 0.8rem; border: 1px solid var(--iwe-accent, #2d6a5e);
    border-radius: var(--iwe-radius-sm, 4px); cursor: pointer;
    background: var(--iwe-accent, #2d6a5e); color: white;
    display: flex; align-items: center; gap: 0.3rem;
  }
  .km-create:hover { opacity: 0.9; }
  .km-create:disabled { opacity: 0.4; cursor: default; }
  .km-delete {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.3rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm, 4px); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: flex; align-items: center; gap: 0.3rem;
  }
  .km-delete:hover { border-color: #b85450; color: #b85450; }
  .km-delete-confirm {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.3rem 0.6rem; border: 1px solid #b85450;
    border-radius: var(--iwe-radius-sm, 4px); cursor: pointer;
    background: #b85450; color: white;
  }
  .km-cancel {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    padding: 0.3rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm, 4px); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
  }
</style>
