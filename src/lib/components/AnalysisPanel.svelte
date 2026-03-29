<script>
  import { wordFrequency, findSimilarPhrases } from '$lib/db.js';

  let { ongotochapter, entities = [] } = $props();

  let subTab = $state('repetition');

  // ---- Repetition finder state ----
  let minLength = $state(4);
  let minCount = $state(3);
  let excludeCharacters = $state(true);
  let excludePlaces = $state(true);
  let excludeThings = $state(true);
  let useWindow = $state(false);
  let windowSize = $state(100);
  let results = $state(null);
  let loading = $state(false);
  let expandedWord = $state(null);
  let filterText = $state('');

  async function runAnalysis() {
    loading = true;
    try {
      results = await wordFrequency(
        minLength,
        minCount,
        useWindow ? windowSize : null
      );
    } catch (e) {
      console.warn('Word frequency failed:', e);
      results = [];
    }
    loading = false;
  }

  function highlightWord(text, word) {
    const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(`(\\b${escaped}\\b)`, 'gi');
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(re, '<mark class="rep-highlight">$1</mark>');
  }

  // Build set of entity words to exclude
  let excludeWords = $derived(() => {
    const words = new Set();
    for (const entity of entities) {
      const shouldExclude =
        (entity.entity_type === 'character' && excludeCharacters) ||
        (entity.entity_type === 'place' && excludePlaces) ||
        (entity.entity_type === 'thing' && excludeThings);
      if (shouldExclude) {
        words.add(entity.name.toLowerCase());
        for (const alias of entity.aliases) {
          // Add each word of multi-word names/aliases
          words.add(alias.toLowerCase());
          for (const part of alias.toLowerCase().split(/\s+/)) {
            if (part.length >= 2) words.add(part);
          }
        }
        // Also add individual words from multi-word entity names
        for (const part of entity.name.toLowerCase().split(/\s+/)) {
          if (part.length >= 2) words.add(part);
        }
      }
    }
    return words;
  });

  let filtered = $derived(() => {
    if (!results) return [];
    const excluded = excludeWords();
    let list = results.filter(r => !excluded.has(r.word));
    if (filterText.trim()) {
      const q = filterText.toLowerCase();
      list = list.filter(r => r.word.includes(q));
    }
    return list;
  });

  // ---- Similar phrasing state ----
  let simMinWords = $state(5);
  let simMinSimilarity = $state(0.6);
  let simResults = $state(null);
  let simLoading = $state(false);

  async function runSimilarScan() {
    simLoading = true;
    try {
      simResults = await findSimilarPhrases(simMinWords, simMinSimilarity);
    } catch (e) {
      console.warn('Similar phrases failed:', e);
      simResults = [];
    }
    simLoading = false;
  }

  function similarityLabel(sim) {
    if (sim >= 0.9) return 'Nearly identical';
    if (sim >= 0.8) return 'Very similar';
    if (sim >= 0.7) return 'Similar';
    return 'Somewhat similar';
  }

  function similarityColor(sim) {
    if (sim >= 0.9) return '#dc2626';
    if (sim >= 0.8) return '#ea580c';
    if (sim >= 0.7) return '#d97706';
    return '#ca8a04';
  }
</script>

<div class="analysis-content">
  <!-- Sub-tabs for future analysis tools -->
  <div class="analysis-sub-tabs">
    <button class="analysis-sub-tab" class:active={subTab === 'repetition'} onclick={() => subTab = 'repetition'}>
      <i class="bi bi-arrow-repeat"></i> Repetition
    </button>
    <button class="analysis-sub-tab" class:active={subTab === 'similar'} onclick={() => subTab = 'similar'}>
      <i class="bi bi-copy"></i> Similar
    </button>
  </div>

  {#if subTab === 'repetition'}
    <div class="rep-controls">
      <div class="rep-row">
        <label class="rep-label">
          Min word length
          <input type="number" class="rep-num" bind:value={minLength} min="2" max="15" />
        </label>
        <label class="rep-label">
          Min occurrences
          <input type="number" class="rep-num" bind:value={minCount} min="2" max="50" />
        </label>
      </div>

      <div class="rep-exclude">
        <span class="rep-exclude-label">Exclude entities:</span>
        <label class="rep-exclude-opt">
          <input type="checkbox" bind:checked={excludeCharacters} />
          Characters
        </label>
        <label class="rep-exclude-opt">
          <input type="checkbox" bind:checked={excludePlaces} />
          Places
        </label>
        <label class="rep-exclude-opt">
          <input type="checkbox" bind:checked={excludeThings} />
          Things
        </label>
      </div>

      <label class="rep-toggle">
        <input type="checkbox" bind:checked={useWindow} />
        <span>Proximity check</span>
      </label>

      {#if useWindow}
        <label class="rep-label">
          Window size
          <div class="rep-slider-row">
            <input type="range" class="rep-slider" bind:value={windowSize} min="20" max="500" step="10" />
            <span class="rep-slider-val">{windowSize} words</span>
          </div>
          <span class="rep-hint">Flag words repeated {minCount}+ times within {windowSize} words of each other</span>
        </label>
      {/if}

      <button class="rep-scan-btn" onclick={runAnalysis} disabled={loading}>
        {#if loading}
          Scanning...
        {:else}
          <i class="bi bi-arrow-repeat"></i> Scan Manuscript
        {/if}
      </button>
    </div>

    {#if results}
      <div class="rep-filter">
        <input class="rep-filter-input" bind:value={filterText} placeholder="Filter words..." />
        <span class="rep-total">{filtered().length} words</span>
      </div>

      <div class="rep-list">
        {#each filtered() as item (item.word)}
          <div class="rep-item" class:expanded={expandedWord === item.word}>
            <button class="rep-item-header" onclick={() => expandedWord = expandedWord === item.word ? null : item.word}>
              <span class="rep-word">{item.word}</span>
              <span class="rep-count">{item.total_count}&times;</span>
              <span class="rep-bar-wrap">
                <span class="rep-bar" style="width: {Math.min(100, (item.total_count / (results[0]?.total_count || 1)) * 100)}%"></span>
              </span>
              <i class="bi" class:bi-chevron-down={expandedWord !== item.word} class:bi-chevron-up={expandedWord === item.word} style="font-size: 0.7rem; color: var(--iwe-text-faint);"></i>
            </button>

            {#if expandedWord === item.word}
              <div class="rep-detail">
                <!-- Chapter breakdown -->
                <div class="rep-chapters">
                  {#each item.chapters.filter(c => c.count > 0).sort((a, b) => b.count - a.count) as ch}
                    <div class="rep-ch-row">
                      <span class="rep-ch-name">{ch.chapter_title}</span>
                      <span class="rep-ch-count">{ch.count}</span>
                    </div>
                  {/each}
                </div>

                <!-- Clusters (if window mode) -->
                {#if item.clusters.length > 0}
                  <div class="rep-clusters-header">
                    <i class="bi bi-exclamation-triangle" style="font-size: 0.7rem;"></i>
                    {item.clusters.length} cluster{item.clusters.length !== 1 ? 's' : ''} found
                  </div>
                  {#each item.clusters as cluster}
                    <button
                      class="rep-cluster"
                      onclick={() => ongotochapter?.(cluster.chapter_id, item.word, cluster.anchor)}
                      title="Jump to this cluster"
                    >
                      <span class="rep-cluster-badge">{cluster.count}&times; in {cluster.chapter_title}</span>
                      <span class="rep-cluster-text">
                        &ldquo;...{@html highlightWord(cluster.context, item.word)}...&rdquo;
                      </span>
                    </button>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

  {:else if subTab === 'similar'}
    <div class="sim-controls">
      <div class="rep-row">
        <label class="rep-label">
          Min sentence length
          <input type="number" class="rep-num" bind:value={simMinWords} min="3" max="20" />
          <span class="rep-hint">words</span>
        </label>
        <label class="rep-label">
          Min similarity
          <input type="range" class="rep-slider" bind:value={simMinSimilarity} min="0.4" max="0.95" step="0.05" />
          <span class="rep-hint">{Math.round(simMinSimilarity * 100)}%</span>
        </label>
      </div>
      <button class="rep-scan-btn" onclick={runSimilarScan} disabled={simLoading}>
        {#if simLoading}
          Scanning...
        {:else}
          <i class="bi bi-copy"></i> Find Similar Phrases
        {/if}
      </button>
    </div>

    <div class="sim-results">
      {#if simLoading}
        <div class="search-empty">Comparing sentences across manuscript...</div>
      {:else if simResults && simResults.length === 0}
        <div class="search-empty">No similar phrases found. Try lowering the similarity threshold.</div>
      {:else if simResults}
        <div class="rep-filter">
          <span class="rep-total">{simResults.length} group{simResults.length !== 1 ? 's' : ''} found</span>
        </div>
        {#each simResults as group, i}
          <div class="sim-group">
            <div class="sim-group-header">
              <span class="sim-badge" style="background: {similarityColor(group.avg_similarity)}">
                {Math.round(group.avg_similarity * 100)}% similar
              </span>
              <span class="sim-count">{group.count} occurrence{group.count !== 1 ? 's' : ''}</span>
            </div>
            {#each group.occurrences as occ}
              <button class="sim-sentence" onclick={() => ongotochapter?.(occ.chapter_id, occ.sentence.split(' ').slice(0, 3).join(' '), occ.anchor)}>
                <div class="sim-sentence-header">
                  <span class="sim-chapter-tag">{occ.chapter_title}</span>
                  {#if occ.similarity < 1.0}
                    <span class="sim-occ-sim">{Math.round(occ.similarity * 100)}%</span>
                  {/if}
                </div>
                <span class="sim-text">&ldquo;{occ.sentence}&rdquo;</span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .analysis-content {
    display: flex; flex-direction: column; height: 100%;
    font-family: var(--iwe-font-ui);
  }

  .analysis-sub-tabs {
    display: flex; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
  }
  .analysis-sub-tab {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    padding: 0.45rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500;
    background: none; border: none; border-bottom: 2px solid transparent;
    color: var(--iwe-text-muted); cursor: pointer; transition: all 150ms;
  }
  .analysis-sub-tab:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-hover); }
  .analysis-sub-tab.active { color: var(--iwe-text); border-bottom-color: var(--iwe-accent); }

  /* Controls */
  .rep-controls {
    padding: 0.6rem 0.75rem;
    display: flex; flex-direction: column; gap: 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .rep-row { display: flex; gap: 0.5rem; }
  .rep-label {
    display: flex; flex-direction: column; gap: 0.15rem;
    font-size: 0.7rem; font-weight: 600; color: var(--iwe-text-muted);
    text-transform: uppercase; letter-spacing: 0.04em; flex: 1;
  }
  .rep-num {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    padding: 0.3rem 0.4rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); width: 100%; outline: none;
  }
  .rep-num:focus { border-color: var(--iwe-accent); }
  .rep-exclude {
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
  }
  .rep-exclude-label {
    font-size: 0.7rem; font-weight: 600; color: var(--iwe-text-muted);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .rep-exclude-opt {
    display: flex; align-items: center; gap: 0.25rem;
    font-size: 0.75rem; color: var(--iwe-text-secondary); cursor: pointer;
  }
  .rep-exclude-opt input { accent-color: var(--iwe-accent); }

  .rep-toggle {
    display: flex; align-items: center; gap: 0.4rem;
    font-size: 0.8rem; color: var(--iwe-text-secondary); cursor: pointer;
  }
  .rep-toggle input { accent-color: var(--iwe-accent); }
  .rep-slider-row { display: flex; align-items: center; gap: 0.4rem; }
  .rep-slider { flex: 1; accent-color: var(--iwe-accent); }
  .rep-slider-val { font-size: 0.75rem; color: var(--iwe-text-secondary); min-width: 65px; text-align: right; }
  .rep-hint { font-size: 0.65rem; color: var(--iwe-text-faint); font-style: italic; text-transform: none; letter-spacing: normal; font-weight: normal; }

  .rep-scan-btn {
    font-family: var(--iwe-font-ui); font-size: 0.85rem; font-weight: 500;
    padding: 0.45rem 0.75rem; border: none;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: var(--iwe-accent); color: white;
    display: flex; align-items: center; justify-content: center; gap: 0.35rem;
    transition: all 150ms;
  }
  .rep-scan-btn:hover:not(:disabled) { background: var(--iwe-accent-hover); }
  .rep-scan-btn:disabled { opacity: 0.4; }

  /* Filter */
  .rep-filter {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .rep-filter-input {
    flex: 1; font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.3rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none;
  }
  .rep-filter-input:focus { border-color: var(--iwe-accent); }
  .rep-total { font-size: 0.7rem; color: var(--iwe-text-faint); white-space: nowrap; }

  /* Results list */
  .rep-list { flex: 1; overflow-y: auto; }

  .rep-item { border-bottom: 1px solid var(--iwe-border-light); }
  .rep-item.expanded { background: var(--iwe-bg-hover); }

  .rep-item-header {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; background: none; border: none;
    padding: 0.45rem 0.75rem; cursor: pointer; text-align: left;
    font-family: var(--iwe-font-ui); transition: background 100ms;
  }
  .rep-item-header:hover { background: var(--iwe-bg-hover); }

  .rep-word {
    font-size: 0.85rem; font-weight: 500; color: var(--iwe-text);
    min-width: 80px;
  }
  .rep-count {
    font-size: 0.75rem; color: var(--iwe-text-muted);
    min-width: 30px; text-align: right;
  }
  .rep-bar-wrap {
    flex: 1; height: 4px; background: var(--iwe-border-light);
    border-radius: 2px; overflow: hidden;
  }
  .rep-bar {
    height: 100%; background: var(--iwe-accent);
    border-radius: 2px; transition: width 0.3s ease;
  }

  /* Detail panel */
  .rep-detail { padding: 0 0.75rem 0.5rem; }
  .rep-chapters {
    display: flex; flex-wrap: wrap; gap: 0.25rem;
    margin-bottom: 0.4rem;
  }
  .rep-ch-row {
    display: flex; align-items: center; gap: 0.25rem;
    font-size: 0.7rem; color: var(--iwe-text-secondary);
    background: var(--iwe-bg); padding: 0.15rem 0.4rem;
    border-radius: 10px; border: 1px solid var(--iwe-border-light);
  }
  .rep-ch-name { font-weight: 500; }
  .rep-ch-count { color: var(--iwe-text-faint); }

  .rep-clusters-header {
    font-size: 0.7rem; font-weight: 600; color: #b45309;
    display: flex; align-items: center; gap: 0.3rem;
    margin: 0.3rem 0 0.2rem;
  }

  .rep-cluster {
    display: block; width: 100%;
    background: none; border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius-sm);
    padding: 0.4rem 0.5rem; margin-bottom: 0.25rem;
    cursor: pointer; text-align: left; transition: background 100ms;
  }
  .rep-cluster:hover { background: var(--iwe-bg); border-color: var(--iwe-accent); }

  .rep-cluster-badge {
    display: block; font-size: 0.65rem; font-weight: 500;
    color: #b45309; margin-bottom: 0.2rem;
  }
  .rep-cluster-text {
    font-family: var(--iwe-font-prose); font-size: 0.8rem;
    color: var(--iwe-text-secondary); line-height: 1.5;
    font-style: italic;
  }

  /* Similar phrasing */
  .sim-controls {
    padding: 0.6rem 0.75rem;
    display: flex; flex-direction: column; gap: 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .sim-results { flex: 1; overflow-y: auto; }
  .sim-group {
    border-bottom: 1px solid var(--iwe-border);
    padding: 0.4rem 0;
  }
  .sim-group-header {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.3rem 0.75rem 0.3rem;
  }
  .sim-badge {
    font-size: 0.65rem; font-weight: 600; color: white;
    padding: 0.15rem 0.5rem; border-radius: 10px;
  }
  .sim-count {
    font-size: 0.75rem; font-weight: 600; color: var(--iwe-text-secondary);
  }
  .sim-sentence {
    display: block; width: 100%; text-align: left;
    background: none; border: none;
    padding: 0.35rem 0.75rem; cursor: pointer;
    transition: background 100ms;
  }
  .sim-sentence:hover { background: var(--iwe-bg-hover); }
  .sim-sentence-header {
    display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.15rem;
  }
  .sim-chapter-tag {
    font-size: 0.6rem; font-weight: 600;
    color: var(--iwe-text-faint); background: var(--iwe-bg-active);
    padding: 0.1rem 0.35rem; border-radius: 3px;
  }
  .sim-occ-sim {
    font-size: 0.6rem; color: var(--iwe-text-faint);
  }
  .sim-text {
    display: block;
    font-family: var(--iwe-font-prose); font-size: 0.85rem;
    color: var(--iwe-text-secondary); line-height: 1.6;
    font-style: italic;
  }

  :global(.rep-highlight) {
    background: #fde68a; color: var(--iwe-text); font-weight: 600;
    border-radius: 2px; padding: 0 2px;
  }
</style>
