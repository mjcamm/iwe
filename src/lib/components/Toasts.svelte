<script>
  import { onMount, onDestroy } from 'svelte';
  import { toasts as toastList, onToastChange } from '$lib/toast.js';

  let toasts = $state([...toastList]);
  let unsub;

  onMount(() => {
    unsub = onToastChange(t => { toasts = [...t]; });
  });

  onDestroy(() => { unsub?.(); });
</script>

{#if toasts.length > 0}
  <div class="toast-container">
    {#each toasts as toast (toast.id)}
      <div class="toast-item toast-{toast.type}">
        {#if toast.type === 'success'}
          <i class="bi bi-check-circle"></i>
        {:else if toast.type === 'error'}
          <i class="bi bi-exclamation-circle"></i>
        {:else}
          <i class="bi bi-info-circle"></i>
        {/if}
        <span>{toast.message}</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed; bottom: 2rem; right: 2rem; z-index: 9999;
    display: flex; flex-direction: column-reverse; gap: 0.5rem;
    pointer-events: none;
  }
  .toast-item {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.6rem 1rem;
    border-radius: var(--iwe-radius);
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    box-shadow: 0 4px 16px rgba(0,0,0,0.12);
    animation: toastSlide 0.25s ease;
    pointer-events: auto;
  }
  @keyframes toastSlide {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .toast-success {
    background: #ecfdf5; color: #065f46; border: 1px solid #a7f3d0;
  }
  .toast-error {
    background: #fef2f2; color: #991b1b; border: 1px solid #fecaca;
  }
  .toast-info {
    background: var(--iwe-bg); color: var(--iwe-text); border: 1px solid var(--iwe-border);
  }
</style>
