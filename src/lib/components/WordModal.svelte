<script>
  import { getSynonyms, addToDictionary } from '$lib/db.js';

  let {
    show = false,
    word = '',
    isMisspelled = false,
    suggestions = [],
    editorFrom = 0,
    editorTo = 0,
    onreplace,
    onclose,
    ondictionaryadd,
  } = $props();

  let synonyms = $state([]);
  let synonymWord = $state('');
  let synonymsLoading = $state(false);
  let synonymsFound = $state(true);
  let filter = $state('');
  let showAll = $state(false);
  let selectedSuggestion = $state('');

  const INITIAL_SHOW = 50;

  // Load synonyms when modal opens or selected suggestion changes
  $effect(() => {
    if (!show) return;
    const lookupWord = selectedSuggestion || (isMisspelled && suggestions.length > 0 ? suggestions[0] : word);
    if (lookupWord) {
      loadSynonyms(lookupWord);
    }
  });

  // Reset state when modal opens
  $effect(() => {
    if (show) {
      filter = '';
      showAll = false;
      selectedSuggestion = '';
    }
  });

  async function loadSynonyms(w) {
    synonymsLoading = true;
    synonymWord = w;
    try {
      const result = await getSynonyms(w);
      synonyms = result.synonyms || [];
      synonymsFound = result.found;
    } catch (e) {
      console.warn('Synonym lookup failed:', e);
      synonyms = [];
      synonymsFound = false;
    }
    synonymsLoading = false;
  }

  function handleReplace(newWord) {
    onreplace?.(newWord, editorFrom, editorTo);
    onclose?.();
  }

  async function handleAddToDictionary() {
    await addToDictionary(word);
    ondictionaryadd?.(word);
    onclose?.();
  }

  function handleSuggestionClick(suggestion) {
    selectedSuggestion = suggestion;
    showAll = false;
    filter = '';
  }

  function handleSuggestionReplace(suggestion) {
    handleReplace(suggestion);
  }

  function handleClose() {
    onclose?.();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      handleClose();
    }
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) {
      handleClose();
    }
  }

  let filteredSynonyms = $derived(() => {
    if (!filter.trim()) return synonyms;
    const f = filter.toLowerCase();
    return synonyms.filter(s => s.toLowerCase().includes(f));
  });

  let displaySynonyms = $derived(() => {
    const filtered = filteredSynonyms();
    if (showAll) return filtered;
    return filtered.slice(0, INITIAL_SHOW);
  });

  let totalFiltered = $derived(() => filteredSynonyms().length);
  let displayCount = $derived(() => displaySynonyms().length);
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="wm-backdrop" onclick={handleBackdropClick}>
    <div class="wm-modal" role="dialog" aria-modal="true" aria-label="Word: {word}">
      <div class="wm-header">
        <div class="wm-title">
          <span class="wm-title-label">Word:</span>
          <span class="wm-title-word">"{word}"</span>
        </div>
        <button class="wm-close" onclick={handleClose} aria-label="Close">&times;</button>
      </div>

      <div class="wm-body">
        {#if isMisspelled}
          <div class="wm-section">
            <div class="wm-section-header">
              <i class="bi bi-spellcheck wm-icon-warn"></i>
              <span>Spelling</span>
            </div>
            <p class="wm-spelling-msg">
              <strong>"{word}"</strong> is not in the dictionary.
            </p>

            {#if suggestions.length > 0}
              <p class="wm-did-you-mean">Did you mean:</p>
              <div class="wm-pills">
                {#each suggestions as sug}
                  <button
                    class="wm-pill wm-pill-suggestion"
                    onclick={() => handleReplace(sug)}
                    title="Replace with {sug}"
                  >{sug}</button>
                {/each}
              </div>
            {:else}
              <p class="wm-no-suggestions">No spelling suggestions found.</p>
            {/if}

            <button class="wm-add-dict-btn" onclick={handleAddToDictionary}>
              Add "{word}" to dictionary
            </button>
          </div>

          <hr class="wm-divider" />
        {/if}

        <div class="wm-section wm-synonym-section">
          <div class="wm-section-header">
            <i class="bi bi-journal-text wm-icon-book"></i>
            <span>Synonyms</span>
            {#if isMisspelled && synonymWord !== word}
              <span class="wm-synonym-note">for "{synonymWord}"</span>
            {/if}
          </div>

          {#if synonymsLoading}
            <p class="wm-loading">Loading synonyms...</p>
          {:else if !synonymsFound || synonyms.length === 0}
            <p class="wm-no-synonyms">No synonyms found for "{synonymWord}".</p>
          {:else}
            <div class="wm-filter-row">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" class="wm-icon-search"><path d="M11.742 10.344a6.5 6.5 0 10-1.397 1.398h-.001l3.85 3.85a1 1 0 001.415-1.414l-3.85-3.85-.017.016zm-5.442.156a5 5 0 110-10 5 5 0 010 10z"/></svg>
              <input
                class="wm-filter-input"
                type="text"
                placeholder="Filter synonyms..."
                bind:value={filter}
              />
              {#if totalFiltered() > INITIAL_SHOW && !showAll}
                <button class="wm-show-all-inline" onclick={() => showAll = true}>
                  Show all {totalFiltered()}
                </button>
              {/if}
            </div>

            <div class="wm-pills wm-synonym-pills">
              {#each displaySynonyms() as syn}
                <button
                  class="wm-pill wm-pill-synonym"
                  onclick={() => handleReplace(syn)}
                  title="Replace with {syn}"
                >{syn}</button>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .wm-backdrop {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0, 0, 0, 0.35);
    display: flex; align-items: center; justify-content: center;
    animation: wm-fade-in 0.15s ease;
  }
  @keyframes wm-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .wm-modal {
    background: var(--iwe-bg, #fff);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.2), 0 4px 16px rgba(0,0,0,0.1);
    width: 90vw; max-width: 680px;
    max-height: 80vh;
    display: flex; flex-direction: column;
    animation: wm-slide-in 0.2s ease;
    overflow: hidden;
  }
  @keyframes wm-slide-in {
    from { opacity: 0; transform: translateY(12px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  .wm-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1.1rem 1.4rem;
    border-bottom: 1px solid var(--iwe-border, #e5e1da);
    flex-shrink: 0;
  }
  .wm-title {
    display: flex; align-items: baseline; gap: 0.5rem;
    font-family: var(--iwe-font-ui, sans-serif);
  }
  .wm-title-label {
    font-size: 0.85rem; color: var(--iwe-text-muted, #9e9891);
    font-weight: 500;
  }
  .wm-title-word {
    font-size: 1.3rem; font-weight: 700;
    color: var(--iwe-text, #2d2a26);
    font-family: var(--iwe-font-prose, serif);
  }
  .wm-close {
    background: none; border: none; cursor: pointer;
    font-size: 1.6rem; line-height: 1; padding: 0.2rem 0.4rem;
    color: var(--iwe-text-faint, #c4bfb8);
    border-radius: var(--iwe-radius-sm, 4px);
    transition: all 100ms;
  }
  .wm-close:hover { color: var(--iwe-text, #2d2a26); background: var(--iwe-bg-hover, #edeae4); }

  .wm-body {
    padding: 1.2rem 1.4rem;
    overflow-y: auto;
    flex: 1;
    display: flex; flex-direction: column;
    min-height: 0;
  }

  .wm-section { margin-bottom: 0.5rem; }
  .wm-synonym-section {
    display: flex; flex-direction: column;
    min-height: 0; flex: 1;
  }
  .wm-section-header {
    display: flex; align-items: center; gap: 0.5rem;
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.8rem; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--iwe-text-secondary, #6b6560);
    margin-bottom: 0.75rem;
    padding: 0.25rem 0 0.35rem;
    flex-shrink: 0;
  }
  .wm-icon-warn { color: var(--iwe-danger, #b85450); font-size: 0.9rem; }
  .wm-icon-book { color: var(--iwe-accent, #2d6a5e); font-size: 0.9rem; }
  .wm-icon-search { color: var(--iwe-text-faint, #c4bfb8); }

  .wm-spelling-msg {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.9rem; color: var(--iwe-text, #2d2a26);
    margin: 0 0 0.6rem;
  }
  .wm-did-you-mean {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.8rem; color: var(--iwe-text-muted, #9e9891);
    margin: 0 0 0.5rem;
  }
  .wm-hint {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.7rem; color: var(--iwe-text-faint, #c4bfb8);
    margin: 0.4rem 0 0.6rem; font-style: italic;
  }
  .wm-no-suggestions, .wm-no-synonyms, .wm-loading {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.85rem; color: var(--iwe-text-muted, #9e9891);
    margin: 0.5rem 0; font-style: italic;
  }

  .wm-pills {
    display: flex; flex-wrap: wrap; gap: 0.4rem;
    margin-bottom: 0.5rem;
    padding-bottom: 0.2rem;
  }
  .wm-synonym-pills {
    flex: 1; overflow-y: auto;
    padding: 0.15rem 0.25rem 0.5rem 0;
    min-height: 0;
  }

  .wm-pill {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.85rem; font-weight: 500;
    padding: 0.4rem 0.75rem;
    border-radius: 20px;
    cursor: pointer;
    border: 1px solid var(--iwe-border, #e5e1da);
    background: var(--iwe-bg, #fff);
    color: var(--iwe-text, #2d2a26);
    transition: all 120ms;
    line-height: 1.2;
  }
  .wm-pill:hover {
    background: var(--iwe-accent-light, #e8f2ef);
    border-color: var(--iwe-accent, #2d6a5e);
    color: var(--iwe-accent, #2d6a5e);
  }
  .wm-pill-suggestion {
    background: var(--iwe-danger-light, #fdf0ef);
    border-color: var(--iwe-danger, #b85450);
    color: var(--iwe-danger, #b85450);
  }
  .wm-pill-suggestion:hover {
    background: var(--iwe-danger, #b85450);
    color: white;
  }
  .wm-pill-suggestion.active {
    background: var(--iwe-danger, #b85450);
    color: white;
    border-color: var(--iwe-danger, #b85450);
  }
  .wm-pill-synonym:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 6px rgba(0,0,0,0.08);
  }

  .wm-add-dict-btn {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.8rem; font-weight: 500;
    padding: 0.45rem 0.9rem;
    border-radius: var(--iwe-radius-sm, 4px);
    cursor: pointer;
    border: 1px solid var(--iwe-border, #e5e1da);
    background: var(--iwe-bg-warm, #faf8f5);
    color: var(--iwe-text-secondary, #6b6560);
    transition: all 120ms;
    margin-top: 0.4rem;
  }
  .wm-add-dict-btn:hover {
    background: var(--iwe-accent-light, #e8f2ef);
    border-color: var(--iwe-accent, #2d6a5e);
    color: var(--iwe-accent, #2d6a5e);
  }

  .wm-divider {
    border: none;
    border-top: 1px solid var(--iwe-border, #e5e1da);
    margin: 1rem 0;
  }

  .wm-show-all-inline {
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.72rem; font-weight: 500;
    padding: 0.2rem 0.5rem;
    border-radius: 12px;
    cursor: pointer;
    border: 1px solid var(--iwe-border, #e5e1da);
    background: none;
    color: var(--iwe-accent, #2d6a5e);
    transition: all 120ms;
    white-space: nowrap; flex-shrink: 0;
  }
  .wm-show-all-inline:hover {
    background: var(--iwe-accent-light, #e8f2ef);
  }
  .wm-synonym-note {
    font-size: 0.72rem; font-weight: 400;
    color: var(--iwe-text-faint, #c4bfb8);
    font-style: italic;
  }

  .wm-filter-row {
    display: flex; align-items: center; gap: 0.5rem;
    margin-bottom: 0.6rem;
    padding: 0.4rem 0.7rem;
    border: 1px solid var(--iwe-border, #e5e1da);
    border-radius: var(--iwe-radius, 6px);
    background: var(--iwe-bg-warm, #faf8f5);
    flex-shrink: 0;
  }
  .wm-filter-input {
    flex: 1; border: none; background: none; outline: none;
    font-family: var(--iwe-font-ui, sans-serif);
    font-size: 0.85rem;
    color: var(--iwe-text, #2d2a26);
  }
  .wm-filter-input::placeholder {
    color: var(--iwe-text-faint, #c4bfb8);
  }
</style>
