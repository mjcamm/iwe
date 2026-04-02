<script>
  import { wordFrequency, findSimilarPhrases, textSearch, adverbAnalysis, debugDialogueSpans } from '$lib/db.js';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

  let { ongotochapter, entities = [], ontoggledialoguehighlight, dialogueHighlightActive = false } = $props();

  let subTab = $state('frequency');
  let selectorOpen = $state(false);

  const analysisTools = [
    { group: 'Repetition', items: [
      { id: 'frequency', icon: 'bi-sort-numeric-down', label: 'Word Frequency' },
      { id: 'clusters', icon: 'bi-arrow-repeat', label: 'Cluster Finder' },
      { id: 'similar', icon: 'bi-copy', label: 'Similar Phrasing' },
    ]},
    { group: 'Style', items: [
      { id: 'adverbs', icon: 'bi-pencil', label: 'Adverb Density' },
      { id: 'readability', icon: 'bi-mortarboard', label: 'Readability Score' },
      { id: 'dialogue-detect', icon: 'bi-chat-quote', label: 'Dialogue Detection' },
    ]},
    { group: 'Overview', items: [
      { id: 'chapters', icon: 'bi-bar-chart', label: 'Chapter Analysis' },
      { id: 'heatmap', icon: 'bi-grid-3x3-gap', label: 'Entity Heatmap' },
      { id: 'pacing', icon: 'bi-activity', label: 'Pacing Analysis' },
    ]},
  ];

  let activeTool = $derived(
    analysisTools.flatMap(g => g.items).find(t => t.id === subTab)
  );

  function selectTool(id) {
    subTab = id;
    selectorOpen = false;
  }

  // ---- Shared exclude state ----
  let excludeCharacters = $state(true);
  let excludePlaces = $state(true);
  let excludeThings = $state(true);

  // ---- Frequency finder state ----
  let freqMinLength = $state(4);
  let freqMinCount = $state(3);
  let freqResults = $state(null);
  let freqLoading = $state(false);
  let freqExpanded = $state(null);
  let freqFilter = $state('');
  let freqOccurrences = $state(null); // search results for expanded word
  let freqOccIdx = $state(0);
  let freqOccLoading = $state(false);

  // ---- Cluster finder state ----
  let clusterMinLength = $state(4);
  let clusterMinCount = $state(3);
  let clusterWindowSize = $state(100);
  let clusterResults = $state(null);
  let clusterLoading = $state(false);
  let clusterExpanded = $state(null);
  let clusterFilter = $state('');

  async function expandFreqWord(word) {
    if (freqExpanded === word) {
      freqExpanded = null;
      freqOccurrences = null;
      return;
    }
    freqExpanded = word;
    freqOccIdx = 0;
    freqOccLoading = true;
    try {
      const result = await textSearch(word, false, true, false, false); // whole word match
      freqOccurrences = result.results || [];
    } catch {
      freqOccurrences = [];
    }
    freqOccLoading = false;
  }

  async function runFrequency() {
    freqLoading = true;
    try {
      freqResults = await wordFrequency(freqMinLength, freqMinCount, null);
    } catch (e) {
      console.warn('Word frequency failed:', e);
      freqResults = [];
    }
    freqLoading = false;
  }

  async function runClusters() {
    clusterLoading = true;
    try {
      clusterResults = await wordFrequency(clusterMinLength, clusterMinCount, clusterWindowSize);
    } catch (e) {
      console.warn('Cluster analysis failed:', e);
      clusterResults = [];
    }
    clusterLoading = false;
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

  let freqFiltered = $derived(() => {
    if (!freqResults) return [];
    const excluded = excludeWords();
    let list = freqResults.filter(r => !excluded.has(r.word));
    if (freqFilter.trim()) {
      const q = freqFilter.toLowerCase();
      list = list.filter(r => r.word.includes(q));
    }
    return list;
  });

  let clusterFiltered = $derived(() => {
    if (!clusterResults) return [];
    const excluded = excludeWords();
    let list = clusterResults.filter(r => !excluded.has(r.word));
    if (clusterFilter.trim()) {
      const q = clusterFilter.toLowerCase();
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

  // ---- Dialogue attribution adverb state ----
  let adverbData = $state(null);
  let adverbLoading = $state(false);
  let adverbExpanded = $state(null); // filter to a specific adverb word
  let adverbShowRedundant = $state(false); // show only redundant instances

  async function runAdverbScan() {
    adverbLoading = true;
    try {
      adverbData = await adverbAnalysis();
    } catch (e) {
      console.warn('Adverb analysis failed:', e);
      adverbData = null;
    }
    adverbLoading = false;
  }

  let adverbFiltered = $derived(() => {
    if (!adverbData) return [];
    let list = adverbData.instances;
    if (adverbShowRedundant) list = list.filter(i => i.redundant);
    if (adverbExpanded) list = list.filter(i => i.adverb.toLowerCase() === adverbExpanded);
    return list;
  });

  let adverbGrouped = $derived(() => {
    const filtered = adverbFiltered();
    const map = new Map();
    for (const inst of filtered) {
      if (!map.has(inst.chapter_id)) {
        map.set(inst.chapter_id, { chapterId: inst.chapter_id, title: inst.chapter_title, instances: [] });
      }
      map.get(inst.chapter_id).instances.push(inst);
    }
    return [...map.values()];
  });

  function highlightAdverb(context, adverb, speechVerb, redundant) {
    let html = context.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    if (!adverb) return html;
    // Highlight adverb
    const advEscaped = adverb.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const advRe = new RegExp(`(\\b${advEscaped}\\b)`, 'gi');
    html = html.replace(advRe, '<mark class="adv-highlight">$1</mark>');
    // For redundant instances, also highlight the speech verb
    if (redundant && speechVerb) {
      const verbEscaped = speechVerb.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const verbRe = new RegExp(`(\\b${verbEscaped}\\b)`, 'gi');
      html = html.replace(verbRe, '<mark class="adv-highlight-verb">$1</mark>');
    }
    return html;
  }

  function similarityLabel(sim) {
    if (sim >= 0.9) return 'Nearly identical';
    if (sim >= 0.8) return 'Very similar';
    if (sim >= 0.7) return 'Similar';
    return 'Somewhat similar';
  }

  // ---- Heatmap state ----
  let heatmapEntities = $state(new Set());

  function toggleHeatmapEntity(id) {
    const next = new Set(heatmapEntities);
    if (next.has(id)) next.delete(id); else next.add(id);
    heatmapEntities = next;
  }

  function selectAllHeatmap() {
    heatmapEntities = new Set(entities.map(e => e.id));
  }

  function selectNoneHeatmap() {
    heatmapEntities = new Set();
  }

  async function launchChapterAnalysis() {
    try {
      new WebviewWindow('chapters-' + Date.now(), {
        url: '/chapters',
        title: 'Chapter Analysis',
        width: 1200,
        height: 800,
        resizable: true,
      });
    } catch (e) {
      console.error('Failed to open chapter analysis:', e);
    }
  }

  async function launchPacing() {
    try {
      new WebviewWindow('pacing-' + Date.now(), {
        url: '/pacing',
        title: 'Pacing Analysis',
        width: 1200,
        height: 800,
        resizable: true,
      });
    } catch (e) {
      console.error('Failed to open pacing analysis:', e);
    }
  }

  async function launchReadability() {
    try {
      new WebviewWindow('readability-' + Date.now(), {
        url: '/readability',
        title: 'Readability Score',
        width: 1200,
        height: 800,
        resizable: true,
      });
    } catch (e) {
      console.error('Failed to open readability analysis:', e);
    }
  }

  async function launchHeatmap() {
    const ids = [...heatmapEntities].join(',');
    try {
      const webview = new WebviewWindow('heatmap-' + Date.now(), {
        url: `/heatmap?entities=${ids}`,
        title: 'Entity Heatmap',
        width: 1200,
        height: 700,
        resizable: true,
      });
      webview.once('tauri://error', (e) => {
        console.error('Heatmap window error:', e);
      });
    } catch (e) {
      console.error('Failed to open heatmap window:', e);
    }
  }

  function similarityColor(sim) {
    if (sim >= 0.9) return '#dc2626';
    if (sim >= 0.8) return '#ea580c';
    if (sim >= 0.7) return '#d97706';
    return '#ca8a04';
  }
</script>

<div class="analysis-content">
  <!-- Analysis tool selector -->
  <div class="tool-selector-wrap">
    <button class="tool-selector-btn" onclick={() => selectorOpen = !selectorOpen}>
      {#if activeTool}
        <i class="bi {activeTool.icon}"></i>
        <span class="tool-selector-label">{activeTool.label}</span>
      {/if}
      <i class="bi bi-chevron-down tool-selector-chevron" class:open={selectorOpen}></i>
    </button>
    {#if selectorOpen}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="tool-selector-backdrop" onclick={() => selectorOpen = false} onkeydown={() => {}}></div>
      <div class="tool-selector-dropdown">
        {#each analysisTools as group}
          <div class="tool-group-label">{group.group}</div>
          {#each group.items as tool (tool.id)}
            <button
              class="tool-option"
              class:active={subTab === tool.id}
              onclick={() => selectTool(tool.id)}
            >
              <i class="bi {tool.icon}"></i>
              <span>{tool.label}</span>
              {#if subTab === tool.id}
                <i class="bi bi-check2 tool-option-check"></i>
              {/if}
            </button>
          {/each}
        {/each}
      </div>
    {/if}
  </div>

  {#if subTab === 'frequency'}
    <!-- Word Frequency -->
    <div class="rep-controls">
      <div class="rep-row">
        <label class="rep-label">
          Min word length
          <input type="number" class="rep-num" bind:value={freqMinLength} min="2" max="15" />
        </label>
        <label class="rep-label">
          Min occurrences
          <input type="number" class="rep-num" bind:value={freqMinCount} min="2" max="50" />
        </label>
      </div>

      <div class="rep-exclude">
        <span class="rep-exclude-label">Exclude entities:</span>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludeCharacters} /> Characters</label>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludePlaces} /> Places</label>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludeThings} /> Things</label>
      </div>

      <button class="rep-scan-btn" onclick={runFrequency} disabled={freqLoading}>
        {#if freqLoading}Scanning...{:else}<i class="bi bi-sort-numeric-down"></i> Scan Manuscript{/if}
      </button>
    </div>

    {#if freqResults}
      <div class="rep-filter">
        <input class="rep-filter-input" bind:value={freqFilter} placeholder="Filter words..." />
        <span class="rep-total">{freqFiltered().length} words</span>
      </div>
      <div class="rep-list">
        {#each freqFiltered() as item (item.word)}
          <div class="rep-item" class:expanded={freqExpanded === item.word}>
            <button class="rep-item-header" onclick={() => expandFreqWord(item.word)}>
              <span class="rep-word">{item.word}</span>
              <span class="rep-count">{item.total_count}&times;</span>
              <span class="rep-bar-wrap">
                <span class="rep-bar" style="width: {Math.min(100, (item.total_count / (freqResults[0]?.total_count || 1)) * 100)}%"></span>
              </span>
              <i class="bi" class:bi-chevron-down={freqExpanded !== item.word} class:bi-chevron-up={freqExpanded === item.word} style="font-size: 0.7rem; color: var(--iwe-text-faint);"></i>
            </button>
            {#if freqExpanded === item.word}
              <div class="rep-detail">
                <div class="rep-chapters">
                  {#each item.chapters.filter(c => c.count > 0).sort((a, b) => b.count - a.count) as ch}
                    <div class="rep-ch-row">
                      <span class="rep-ch-name">{ch.chapter_title}</span>
                      <span class="rep-ch-count">{ch.count}</span>
                    </div>
                  {/each}
                </div>

                {#if freqOccLoading}
                  <div class="occ-loading">Loading occurrences...</div>
                {:else if freqOccurrences && freqOccurrences.length > 0}
                  {@const occ = freqOccurrences[freqOccIdx]}
                  <div class="occ-browser">
                    <div class="occ-nav">
                      <button class="occ-nav-btn" onclick={() => { if (freqOccIdx > 0) freqOccIdx--; }} disabled={freqOccIdx === 0}>
                        <i class="bi bi-chevron-left"></i>
                      </button>
                      <span class="occ-counter">{freqOccIdx + 1} / {freqOccurrences.length}</span>
                      <button class="occ-nav-btn" onclick={() => { if (freqOccIdx < freqOccurrences.length - 1) freqOccIdx++; }} disabled={freqOccIdx >= freqOccurrences.length - 1}>
                        <i class="bi bi-chevron-right"></i>
                      </button>
                    </div>
                    <div class="occ-chapter-tag">{occ.chapter_title}</div>
                    <button
                      class="occ-snippet"
                      onclick={() => ongotochapter?.(occ.chapter_id, item.word, occ.char_position)}
                      title="Jump to this occurrence"
                    >
                      &ldquo;...{@html highlightWord(occ.context, item.word)}...&rdquo;
                    </button>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

  {:else if subTab === 'clusters'}
    <!-- Cluster Finder -->
    <div class="rep-controls">
      <div class="rep-row">
        <label class="rep-label">
          Min word length
          <input type="number" class="rep-num" bind:value={clusterMinLength} min="2" max="15" />
        </label>
        <label class="rep-label">
          Min to cluster
          <input type="number" class="rep-num" bind:value={clusterMinCount} min="2" max="20" />
        </label>
      </div>

      <label class="rep-label">
        Window size
        <div class="rep-slider-row">
          <input type="range" class="rep-slider" bind:value={clusterWindowSize} min="20" max="500" step="10" />
          <span class="rep-slider-val">{clusterWindowSize} words</span>
        </div>
        <span class="rep-hint">Flag words appearing {clusterMinCount}+ times within {clusterWindowSize} words</span>
      </label>

      <div class="rep-exclude">
        <span class="rep-exclude-label">Exclude entities:</span>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludeCharacters} /> Characters</label>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludePlaces} /> Places</label>
        <label class="rep-exclude-opt"><input type="checkbox" bind:checked={excludeThings} /> Things</label>
      </div>

      <button class="rep-scan-btn" onclick={runClusters} disabled={clusterLoading}>
        {#if clusterLoading}Scanning...{:else}<i class="bi bi-arrow-repeat"></i> Find Clusters{/if}
      </button>
    </div>

    {#if clusterResults}
      <div class="rep-filter">
        <input class="rep-filter-input" bind:value={clusterFilter} placeholder="Filter words..." />
        <span class="rep-total">{clusterFiltered().length} words with clusters</span>
      </div>
      <div class="rep-list">
        {#each clusterFiltered() as item (item.word)}
          <div class="rep-item" class:expanded={clusterExpanded === item.word}>
            <button class="rep-item-header" onclick={() => clusterExpanded = clusterExpanded === item.word ? null : item.word}>
              <span class="rep-word">{item.word}</span>
              <span class="rep-count">{item.total_count}&times; total</span>
              <span class="rep-cluster-count">{item.clusters.length} cluster{item.clusters.length !== 1 ? 's' : ''}</span>
              <i class="bi" class:bi-chevron-down={clusterExpanded !== item.word} class:bi-chevron-up={clusterExpanded === item.word} style="font-size: 0.7rem; color: var(--iwe-text-faint);"></i>
            </button>
            {#if clusterExpanded === item.word}
              <div class="rep-detail">
                {#each item.clusters as cluster}
                  <button
                    class="rep-cluster"
                    onclick={() => ongotochapter?.(cluster.chapter_id, item.word, cluster.char_start)}
                    title="Jump to this cluster"
                  >
                    <span class="rep-cluster-badge">{cluster.count}&times; within {cluster.window_words} words — {cluster.chapter_title}</span>
                    <span class="rep-cluster-text">
                      &ldquo;...{@html highlightWord(cluster.context, item.word)}...&rdquo;
                    </span>
                  </button>
                {/each}
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
              <button class="sim-sentence" onclick={() => ongotochapter?.(occ.chapter_id, occ.sentence, occ.char_position)}>
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

  {:else if subTab === 'chapters'}
    <div class="hm-setup">
      <p class="hm-setup-desc">Opens a full-screen dashboard with word counts, dialogue vs narrative breakdown, sentence length analysis, vocabulary density, and a detailed chapter comparison table.</p>
      <button class="rep-scan-btn" onclick={launchChapterAnalysis}>
        <i class="bi bi-bar-chart"></i> Open Chapter Analysis
      </button>
    </div>

  {:else if subTab === 'heatmap'}
    <div class="hm-setup">
      <div class="hm-setup-header">
        <span>Select entities to visualise:</span>
        <div class="hm-select-actions">
          <button class="hm-select-btn" onclick={selectAllHeatmap}>All</button>
          <button class="hm-select-btn" onclick={selectNoneHeatmap}>None</button>
        </div>
      </div>

      <div class="hm-entity-list">
        {#each entities as entity (entity.id)}
          <label class="hm-entity-opt">
            <input type="checkbox" checked={heatmapEntities.has(entity.id)} onchange={() => toggleHeatmapEntity(entity.id)} />
            <span class="hm-entity-dot" style="background: {entity.color}"></span>
            <span class="hm-entity-name">{entity.name}</span>
            <span class="hm-entity-type">{entity.entity_type}</span>
          </label>
        {/each}
      </div>

      <button class="rep-scan-btn" onclick={launchHeatmap} disabled={heatmapEntities.size === 0}>
        <i class="bi bi-grid-3x3-gap"></i>
        Open Heatmap ({heatmapEntities.size} entities)
      </button>
    </div>

  {:else if subTab === 'pacing'}
    <div class="hm-setup">
      <p class="hm-setup-desc">Visualise the rhythm of your prose. Sentence length waveforms show where your writing builds tension, slows down, or flatlines.</p>
      <button class="rep-scan-btn" onclick={launchPacing}>
        <i class="bi bi-activity"></i> Open Pacing Analysis
      </button>
    </div>

  {:else if subTab === 'readability'}
    <div class="hm-setup">
      <p class="hm-setup-desc">Flesch-Kincaid grade level measures prose complexity based on sentence length and syllable count. Consistency across chapters matters more than any single number.</p>
      <button class="rep-scan-btn" onclick={launchReadability}>
        <i class="bi bi-mortarboard"></i> Open Readability Score
      </button>
    </div>

  {:else if subTab === 'dialogue-detect'}
    <div class="hm-setup">
      <p class="hm-setup-desc">Highlights all detected dialogue in the current chapter. Use this to verify dialogue detection is working correctly and spot missing or mismatched quotation marks.</p>
      <button
        class="rep-scan-btn"
        class:active={dialogueHighlightActive}
        onclick={() => ontoggledialoguehighlight?.()}
        style={dialogueHighlightActive ? 'background: #dc2626;' : ''}
      >
        <i class="bi" class:bi-chat-quote={!dialogueHighlightActive} class:bi-x-lg={dialogueHighlightActive}></i>
        {dialogueHighlightActive ? 'Clear Highlights' : 'Highlight Dialogue'}
      </button>
      {#if dialogueHighlightActive}
        <p class="dialogue-detect-hint">Dialogue is highlighted with a <span class="dialogue-detect-sample">teal underline</span> in the editor. Text without highlighting is detected as narration. Check for missing or mismatched quotes.</p>
      {/if}
    </div>

  {:else if subTab === 'adverbs'}
    <div class="adv-quote">
      <p class="adv-quote-text">"I believe the road to hell is paved with adverbs, and I will shout it from the rooftops. To put it another way, they're like dandelions. If you have one on your lawn, it looks pretty and unique. If you fail to root it out, however, you find five the next day... fifty the day after that... and then, my brothers and sisters, your lawn is totally, completely, and profligately covered with dandelions."</p>
      <p class="adv-quote-attr">-- Stephen King, <em>On Writing: A Memoir of the Craft</em></p>
    </div>
    <div class="rep-controls">
      <p class="adv-desc">Finds adverbs in dialogue attribution -- where the author tells the reader how something was said instead of showing it.</p>
      <button class="rep-scan-btn" onclick={runAdverbScan} disabled={adverbLoading}>
        {#if adverbLoading}Scanning...{:else}<i class="bi bi-pencil"></i> Scan Dialogue Tags{/if}
      </button>
    </div>

    {#if adverbData}
      <!-- Summary stats -->
      {@const adverbRate = adverbData.total_dialogue_spans > 0 ? (adverbData.attributions_with_adverbs / adverbData.total_dialogue_spans * 100) : 0}
      <div class="adv-summary">
        <div class="adv-stat">
          <span class="adv-stat-val">{adverbRate.toFixed(1)}%</span>
          <span class="adv-stat-label">Tag adverb rate</span>
        </div>
        <div class="adv-stat">
          <span class="adv-stat-val">{adverbData.total_instances}</span>
          <span class="adv-stat-label">Adverbs found</span>
        </div>
        <div class="adv-stat">
          <span class="adv-stat-val">{adverbData.total_dialogue_spans}</span>
          <span class="adv-stat-label">Dialogue spans</span>
        </div>
        <div class="adv-stat">
          <span class="adv-stat-val adv-dialogue-tag">{adverbData.redundant_count}</span>
          <span class="adv-stat-label">Redundant</span>
        </div>
      </div>

      <!-- Filter: redundant only -->
      {#if adverbData.redundant_count > 0}
        <div class="adv-filters">
          <button class="adv-filter-btn" class:active={!adverbShowRedundant} onclick={() => adverbShowRedundant = false}>
            All ({adverbData.total_instances})
          </button>
          <button class="adv-filter-btn" class:active={adverbShowRedundant} onclick={() => adverbShowRedundant = true}>
            Redundant ({adverbData.redundant_count})
          </button>
        </div>
      {/if}

      <!-- Top offenders -->
      <div class="adv-top-section">
        <div class="adv-top-label">Most frequent</div>
        <div class="adv-top-list">
          {#each adverbData.top_adverbs.slice(0, 15) as freq (freq.word)}
            <button
              class="adv-top-pill"
              class:active={adverbExpanded === freq.word}
              onclick={() => adverbExpanded = adverbExpanded === freq.word ? null : freq.word}
            >
              {freq.word}
              <span class="adv-pill-count">{freq.count}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Instance list -->
      <div class="adv-instance-list">
        {#if adverbFiltered().length === 0}
          <div class="search-empty">No matches for current filter.</div>
        {:else}
          <div class="adv-result-count">
            {adverbFiltered().length} instance{adverbFiltered().length !== 1 ? 's' : ''}
            across {adverbGrouped().length} chapter{adverbGrouped().length !== 1 ? 's' : ''}
          </div>
        {/if}
        {#each adverbGrouped() as chapter (chapter.chapterId)}
          <div class="search-chapter">
            <div class="search-chapter-header">
              <span class="search-chapter-title">{chapter.title}</span>
              <span class="search-chapter-count">{chapter.instances.length}</span>
            </div>
            {#each chapter.instances as inst}
              <button
                class="search-slab"
                onclick={async () => {
                  console.log('[adverb-nav]', {
                    adverb: inst.adverb,
                    speechVerb: inst.speech_verb,
                    chapterId: inst.chapter_id,
                    chapterTitle: inst.chapter_title,
                    charPosition: inst.char_position,
                    redundant: inst.redundant,
                    dialogueSnippet: inst.dialogue_snippet,
                    context: inst.context,
                  });
                  try {
                    const debug = await debugDialogueSpans(inst.chapter_id, inst.char_position);
                    console.log('[dialogue-debug]', debug);
                  } catch (e) { console.warn('[dialogue-debug] failed:', e); }
                  ongotochapter?.(inst.chapter_id, inst.adverb, inst.char_position);
                }}
                title="Jump to this adverb"
              >
                <div class="adv-slab-meta">
                  <span class="adv-tag-badge">{inst.speech_verb}</span>
                  {#if inst.redundant}
                    <span class="adv-redundant-badge">redundant</span>
                  {/if}
                </div>
                <span class="slab-text">
                  ...{@html highlightAdverb(inst.context, inst.adverb, inst.speech_verb, inst.redundant)}...
                </span>
              </button>
            {/each}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .analysis-content {
    display: flex; flex-direction: column; height: 100%;
    font-family: var(--iwe-font-ui);
  }

  .tool-selector-wrap {
    position: relative; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
  }
  .tool-selector-btn {
    display: flex; align-items: center; gap: 0.4rem;
    width: 100%; padding: 0.5rem 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem; font-weight: 500;
    background: none; border: none; color: var(--iwe-text);
    cursor: pointer; transition: background 100ms;
  }
  .tool-selector-btn:hover { background: var(--iwe-bg-hover); }
  .tool-selector-label { flex: 1; text-align: left; }
  .tool-selector-chevron {
    font-size: 0.65rem; color: var(--iwe-text-faint);
    transition: transform 150ms;
  }
  .tool-selector-chevron.open { transform: rotate(180deg); }

  .tool-selector-backdrop {
    position: fixed; inset: 0; z-index: 99;
  }
  .tool-selector-dropdown {
    position: absolute; top: 100%; left: 0; right: 0; z-index: 100;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-top: none; border-radius: 0 0 var(--iwe-radius-sm) var(--iwe-radius-sm);
    box-shadow: 0 4px 12px rgba(0,0,0,0.1);
    padding: 0.25rem 0;
  }
  .tool-group-label {
    font-size: 0.6rem; font-weight: 700; color: var(--iwe-text-faint);
    text-transform: uppercase; letter-spacing: 0.06em;
    padding: 0.5rem 0.75rem 0.2rem;
  }
  .tool-option {
    display: flex; align-items: center; gap: 0.4rem;
    width: 100%; padding: 0.4rem 0.75rem 0.4rem 1.1rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    background: none; border: none; color: var(--iwe-text-secondary);
    cursor: pointer; text-align: left; transition: all 100ms;
  }
  .tool-option:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .tool-option.active { color: var(--iwe-accent); font-weight: 500; }
  .tool-option-check { margin-left: auto; font-size: 0.75rem; }

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

  /* Heatmap setup */
  .hm-setup {
    padding: 0.6rem 0.75rem;
    display: flex; flex-direction: column; gap: 0.5rem;
  }
  .hm-setup-header {
    display: flex; align-items: center; justify-content: space-between;
    font-size: 0.8rem; color: var(--iwe-text-secondary); font-weight: 500;
  }
  .hm-select-actions { display: flex; gap: 0.3rem; }
  .hm-select-btn {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    padding: 0.15rem 0.4rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
  }
  .hm-setup-desc {
    font-size: 0.8rem; color: var(--iwe-text-secondary); line-height: 1.5;
    margin: 0 0 0.5rem;
  }
  .hm-select-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  .hm-entity-list {
    max-height: 300px; overflow-y: auto;
    border: 1px solid var(--iwe-border-light); border-radius: var(--iwe-radius-sm);
    padding: 0.25rem;
  }
  .hm-entity-opt {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.3rem 0.4rem; cursor: pointer; border-radius: var(--iwe-radius-sm);
    transition: background 100ms;
  }
  .hm-entity-opt:hover { background: var(--iwe-bg-hover); }
  .hm-entity-opt input { accent-color: var(--iwe-accent); }
  .hm-entity-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .hm-entity-name { font-size: 0.85rem; color: var(--iwe-text); flex: 1; }
  .hm-entity-type { font-size: 0.65rem; color: var(--iwe-text-faint); text-transform: capitalize; }

  /* Occurrence browser */
  .occ-loading { font-size: 0.75rem; color: var(--iwe-text-faint); font-style: italic; padding: 0.3rem 0; }
  .occ-browser {
    margin-top: 0.4rem; border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius-sm); overflow: hidden;
  }
  .occ-nav {
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    padding: 0.3rem; background: var(--iwe-bg-sidebar);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .occ-nav-btn {
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text-secondary); padding: 0.15rem 0.4rem;
    display: flex; align-items: center; transition: all 100ms;
  }
  .occ-nav-btn:hover:not(:disabled) { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .occ-nav-btn:disabled { opacity: 0.25; cursor: default; }
  .occ-counter { font-size: 0.75rem; color: var(--iwe-text-muted); font-weight: 500; }
  .occ-chapter-tag {
    font-size: 0.65rem; font-weight: 600; color: var(--iwe-text-faint);
    background: var(--iwe-bg-active); padding: 0.2rem 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .occ-snippet {
    display: block; width: 100%; text-align: left;
    background: none; border: none; padding: 0.5rem;
    font-family: var(--iwe-font-prose); font-size: 0.8rem;
    color: var(--iwe-text-secondary); line-height: 1.6;
    cursor: pointer; transition: background 100ms;
  }
  .occ-snippet:hover { background: var(--iwe-bg-hover); }

  .rep-cluster-count {
    font-size: 0.65rem; color: #b45309; font-weight: 600;
    background: #fef3c7; padding: 0.1rem 0.35rem; border-radius: 8px;
  }

  :global(.rep-highlight) {
    background: #fde68a; color: var(--iwe-text); font-weight: 600;
    border-radius: 2px; padding: 0 2px;
  }

  /* Adverb quote */
  .adv-quote {
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
    border-left: 3px solid var(--iwe-accent);
    background: var(--iwe-bg-hover);
    margin: 0;
  }
  .adv-quote-text {
    font-family: var(--iwe-font-prose); font-size: 0.9rem;
    color: var(--iwe-text-secondary); line-height: 1.6;
    font-style: italic; margin: 0 0 0.3rem;
  }
  .adv-quote-attr {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    color: var(--iwe-text-secondary); margin: 0;
  }

  .dialogue-detect-hint {
    font-size: 0.8rem; color: var(--iwe-text-secondary); line-height: 1.5; margin: 0;
  }
  .dialogue-detect-sample {
    border-bottom: 2px solid #2d6a5e; background: rgba(45, 106, 94, 0.15);
    padding: 0 2px;
  }
  .rep-scan-btn.active { background: #dc2626; }
  .rep-scan-btn.active:hover { background: #b91c1c; }

  .adv-desc {
    font-size: 0.9rem; color: var(--iwe-text-secondary);
    line-height: 1.5; margin: 0 0 0.3rem;
  }

  /* Adverb density */
  .adv-summary {
    display: flex; gap: 1px; background: var(--iwe-border-light);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .adv-stat {
    flex: 1; display: flex; flex-direction: column; align-items: center;
    padding: 0.5rem; background: var(--iwe-bg);
  }
  .adv-stat-val {
    font-size: 1.1rem; font-weight: 600; color: var(--iwe-text);
    font-family: var(--iwe-font-prose);
  }
  .adv-stat-val.adv-dialogue-tag { color: #dc2626; }
  .adv-stat-label {
    font-size: 0.6rem; color: var(--iwe-text-faint); text-transform: uppercase;
    letter-spacing: 0.04em; margin-top: 0.1rem;
  }

  .adv-filters {
    display: flex; border-bottom: 1px solid var(--iwe-border-light);
  }
  .adv-filter-btn {
    flex: 1; padding: 0.35rem 0.3rem; background: none; border: none;
    border-bottom: 2px solid transparent;
    font-family: var(--iwe-font-ui); font-size: 0.7rem; font-weight: 500;
    color: var(--iwe-text-muted); cursor: pointer; transition: all 100ms;
  }
  .adv-filter-btn:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-hover); }
  .adv-filter-btn.active { color: var(--iwe-text); border-bottom-color: var(--iwe-accent); }

  .adv-top-section {
    padding: 0.4rem 0.75rem; border-bottom: 1px solid var(--iwe-border-light);
  }
  .adv-top-label {
    font-size: 0.65rem; font-weight: 600; color: var(--iwe-text-faint);
    text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 0.3rem;
  }
  .adv-top-list { display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .adv-top-pill {
    display: flex; align-items: center; gap: 0.25rem;
    padding: 0.2rem 0.5rem; border-radius: 12px;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    background: var(--iwe-bg-hover); border: 1px solid var(--iwe-border-light);
    color: var(--iwe-text-secondary); cursor: pointer; transition: all 100ms;
  }
  .adv-top-pill:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .adv-top-pill.active { background: var(--iwe-accent); color: white; border-color: var(--iwe-accent); }
  .adv-top-pill.active .adv-pill-count,
  .adv-top-pill.active .adv-pill-dlg { color: rgba(255,255,255,0.7); }
  .adv-pill-count { font-size: 0.65rem; color: var(--iwe-text-faint); font-weight: 600; }
  .adv-pill-dlg { font-size: 0.6rem; color: #dc2626; font-weight: 500; }

  .adv-instance-list { flex: 1; overflow-y: auto; }
  .adv-result-count {
    font-size: 0.7rem; color: var(--iwe-text-faint); padding: 0.4rem 0.75rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .adv-slab-meta {
    display: flex; align-items: center; gap: 0.3rem; margin-bottom: 0.2rem;
  }
  .adv-tag-badge {
    font-size: 0.6rem; color: var(--iwe-text-faint); background: var(--iwe-bg-active);
    padding: 0.1rem 0.3rem; border-radius: 8px; font-weight: 500;
  }
  .adv-redundant-badge {
    font-size: 0.55rem; color: #dc2626; background: #fef2f2;
    padding: 0.1rem 0.3rem; border-radius: 8px; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.03em;
  }
  .search-chapter { border-bottom: 1px solid var(--iwe-border); }
  .search-chapter-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.5rem 0.75rem; position: sticky; top: 0; z-index: 1;
    background: var(--iwe-bg-sidebar);
    border-bottom: 1px solid var(--iwe-border-light);
  }
  .search-chapter-title {
    font-family: var(--iwe-font-prose);
    font-size: 0.95rem; font-weight: 600; color: var(--iwe-text);
  }
  .search-chapter-count {
    font-size: 0.7rem; color: var(--iwe-text-muted);
    background: var(--iwe-bg-active); padding: 0.15rem 0.45rem;
    border-radius: 10px; font-weight: 500;
  }
  .search-slab {
    display: block; width: 100%;
    background: none; border: none; border-bottom: 1px solid var(--iwe-border-light);
    padding: 0.65rem 0.75rem; cursor: pointer; text-align: left;
    transition: background 100ms; font-family: var(--iwe-font-ui);
  }
  .search-slab:last-child { border-bottom: none; }
  .search-slab:hover { background: var(--iwe-bg-hover); }
  .slab-text {
    font-family: var(--iwe-font-prose); font-size: 0.85rem;
    color: var(--iwe-text-secondary); line-height: 1.7;
  }
  .search-empty {
    padding: 1.5rem; text-align: center; color: var(--iwe-text-faint);
    font-size: 0.85rem; font-style: italic;
  }
  :global(.adv-highlight) {
    background: #fde68a; color: var(--iwe-text); font-weight: 600;
    border-radius: 2px; padding: 0 2px;
  }
  :global(.adv-highlight-verb) {
    background: #fecaca; color: var(--iwe-text); font-weight: 600;
    border-radius: 2px; padding: 0 2px;
  }
</style>
