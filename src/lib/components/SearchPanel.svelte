<script>
  import { onMount, tick } from 'svelte';
  import { relationshipSearch, textSearch, dialogueSearch, semanticSearch, rebuildSemanticIndex, getSemanticIndexStatus } from '$lib/db.js';
  import { listen } from '@tauri-apps/api/event';

  let { entities = [], ongotochapter } = $props();

  let subTab = $state('text'); // 'text' | 'dialogue' | 'relationship' | 'semantic'
  let selectorOpen = $state(false);
  let textSearchInput = $state(null);

  // Called by the parent when the user presses Ctrl/Cmd+F. Switches to the Text
  // Search sub-tab, focuses the query input, and selects any existing text so
  // the user can immediately type over it.
  export async function focusTextSearch() {
    subTab = 'text';
    await tick();
    textSearchInput?.focus();
    textSearchInput?.select();
  }

  const searchTools = [
    { group: 'Search', items: [
      { id: 'text', icon: 'bi-fonts', label: 'Text Search' },
      { id: 'dialogue', icon: 'bi-chat-quote', label: 'Dialogue Search' },
      { id: 'relationship', icon: 'bi-diagram-3', label: 'Relationship Search' },
      { id: 'semantic', icon: 'bi-stars', label: 'Descriptive Search' },
    ]},
  ];

  let activeTool = $derived(
    searchTools.flatMap(g => g.items).find(t => t.id === subTab)
  );

  function selectTool(id) {
    subTab = id;
    selectorOpen = false;
  }

  // ---- Text search state ----
  let textQuery = $state('');
  let searchMode = $state('normal'); // 'normal' | 'case' | 'whole' | 'regex' | 'fuzzy'
  let textResults = $state(null);
  let textSearching = $state(false);

  async function runTextSearch() {
    if (!textQuery.trim()) return;
    textSearching = true;
    try {
      textResults = await textSearch(
        textQuery,
        searchMode === 'case',
        searchMode === 'whole',
        searchMode === 'regex',
        searchMode === 'fuzzy'
      );
    } catch (e) {
      console.warn('Text search failed:', e);
      textResults = { total_matches: 0, results: [] };
    }
    textSearching = false;
  }

  function handleTextKeydown(e) {
    if (e.key === 'Enter') runTextSearch();
  }

  function highlightTextMatch(context, matchedText) {
    if (!matchedText) return escapeHtml(context);
    try {
      // Escape the matched text for use in regex, then highlight all occurrences
      const escaped = matchedText.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`(${escaped})`, 'gi');
      return escapeHtml(context).replace(re, '<mark class="search-highlight">$1</mark>');
    } catch {
      return escapeHtml(context);
    }
  }

  // Group text results by chapter
  let textGrouped = $derived(() => {
    if (!textResults?.results) return [];
    const map = new Map();
    for (const r of textResults.results) {
      if (!map.has(r.chapter_id)) {
        map.set(r.chapter_id, { title: r.chapter_title, matchCount: r.match_count, results: [] });
      }
      map.get(r.chapter_id).results.push(r);
    }
    return [...map.entries()].map(([id, data]) => ({ chapterId: id, ...data }));
  });

  // ---- Dialogue search state ----
  let dlgQuery = $state('');
  let dlgMode = $state('normal');
  let dlgResults = $state(null);
  let dlgSearching = $state(false);

  async function runDialogueSearch() {
    if (!dlgQuery.trim()) return;
    dlgSearching = true;
    try {
      dlgResults = await dialogueSearch(
        dlgQuery,
        dlgMode === 'case',
        dlgMode === 'whole',
        dlgMode === 'regex',
        dlgMode === 'fuzzy'
      );
    } catch (e) {
      console.warn('Dialogue search failed:', e);
      dlgResults = [];
    }
    dlgSearching = false;
  }

  function handleDlgKeydown(e) {
    if (e.key === 'Enter') runDialogueSearch();
  }

  let dlgGrouped = $derived(() => {
    if (!dlgResults || !Array.isArray(dlgResults)) return [];
    const map = new Map();
    for (const r of dlgResults) {
      if (!map.has(r.chapter_id)) {
        map.set(r.chapter_id, { title: r.chapter_title, results: [] });
      }
      map.get(r.chapter_id).results.push(r);
    }
    return [...map.entries()].map(([id, data]) => ({ chapterId: id, ...data }));
  });

  function highlightDialogue(context, matchedText, dialogue) {
    let html = escapeHtml(context);
    // Highlight the matched word within the dialogue
    if (matchedText) {
      const escaped = matchedText.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`(${escaped})`, 'gi');
      html = html.replace(re, '<mark class="search-highlight">$1</mark>');
    }
    // Dim non-dialogue text
    const escapedDialogue = escapeHtml(dialogue);
    if (escapedDialogue && html.includes(escapedDialogue)) {
      // Already has highlights inside, skip wrapping
    }
    return html;
  }

  // ---- Relationship search state ----
  let entityAId = $state(null);
  let entityBId = $state(null);
  let searchType = $state('near');
  let maxDistance = $state(1000);
  let relResults = $state([]);
  let relSearching = $state(false);
  let relHasSearched = $state(false);
  let expandedMiddles = $state(new Set());

  async function runRelSearch() {
    const aId = Number(entityAId);
    const bId = Number(entityBId);
    if (!aId || !bId) return;
    relSearching = true;
    relHasSearched = true;
    try {
      relResults = await relationshipSearch(aId, bId, searchType, Number(maxDistance));
    } catch (e) {
      console.warn('Relationship search failed:', e);
      relResults = [];
    }
    relSearching = false;
  }

  function toggleMiddle(key) {
    const next = new Set(expandedMiddles);
    if (next.has(key)) next.delete(key); else next.add(key);
    expandedMiddles = next;
  }

  function highlightEntities(text) {
    if (!entityAId || !entityBId) return escapeHtml(text);
    const entityA = entities.find(e => e.id === Number(entityAId));
    const entityB = entities.find(e => e.id === Number(entityBId));
    if (!entityA || !entityB) return escapeHtml(text);

    const terms = [];
    [entityA, entityB].forEach(entity => {
      terms.push({ text: entity.name, color: entity.color });
      entity.aliases.forEach(a => terms.push({ text: a, color: entity.color }));
    });
    terms.sort((a, b) => b.text.length - a.text.length);

    const pattern = terms.map(t => t.text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
    const regex = new RegExp(`(${pattern})`, 'gi');
    return escapeHtml(text).replace(regex, (match) => {
      const term = terms.find(t => t.text.toLowerCase() === match.toLowerCase());
      const color = term ? term.color : '#666';
      return `<mark style="background: ${color}30; color: ${color}; font-weight: 600; border-radius: 2px; padding: 0 2px;">${match}</mark>`;
    });
  }

  let relGrouped = $derived(() => {
    const map = new Map();
    for (const r of relResults) {
      if (!map.has(r.chapter_id)) {
        map.set(r.chapter_id, { title: r.chapter_title, results: [] });
      }
      map.get(r.chapter_id).results.push(r);
    }
    return [...map.entries()].map(([id, data]) => ({ chapterId: id, ...data }));
  });

  // ---- Semantic search state ----
  let semQuery = $state('');
  let semGranularity = $state('sentence');
  let semResults = $state(null);
  let semSearching = $state(false);
  let semIndexStatus = $state(null);
  let semRebuilding = $state(false);
  let semThreshold = $state(50); // minimum score % to show

  async function runMeaningSearch() {
    if (!semQuery.trim()) return;
    semSearching = true;
    try {
      semResults = await semanticSearch(semQuery.trim(), semGranularity, 20);
    } catch (e) {
      console.warn('[semantic] search failed:', e);
      semResults = [];
    }
    semSearching = false;
  }

  async function loadIndexStatus() {
    try {
      semIndexStatus = await getSemanticIndexStatus();
    } catch { semIndexStatus = null; }
  }

  async function handleRebuildIndex() {
    semRebuilding = true;
    try {
      await rebuildSemanticIndex();
      loadIndexStatus();
    } catch (e) {
      console.warn('[semantic] rebuild failed:', e);
      semRebuilding = false;
    }
    // semRebuilding is cleared by the semantic-index-updated event listener
  }

  let semProgress = $state(null); // { done, total } during indexing

  let semFiltered = $derived(
    semResults ? semResults.filter(r => Math.round(r.score * 100) >= semThreshold) : []
  );

  let semGrouped = $derived(() => {
    const map = new Map();
    for (const r of semFiltered) {
      if (!map.has(r.chapter_id)) {
        map.set(r.chapter_id, { chapterId: r.chapter_id, title: r.chapter_title, results: [] });
      }
      map.get(r.chapter_id).results.push(r);
    }
    return [...map.values()];
  });

  // Load index status when switching to semantic tab
  $effect(() => {
    if (subTab === 'semantic') loadIndexStatus();
  });

  // Listen for indexing events
  onMount(() => {
    listen('semantic-index-updated', () => {
      console.log('[semantic] index updated event received');
      loadIndexStatus();
      semProgress = null;
      semRebuilding = false;
    });
    listen('semantic-index-progress', (event) => {
      console.log('[semantic] progress:', event.payload);
      semProgress = event.payload;
      loadIndexStatus();
    });
  });

  function escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
</script>

<div class="search-panel">
  <!-- Tool selector dropdown -->
  <div class="tool-selector-wrap">
    <button class="tool-selector-btn" onclick={() => selectorOpen = !selectorOpen}>
      {#if activeTool}
        <i class="bi {activeTool.icon}"></i>
        <span class="tool-selector-label">{activeTool.label}</span>
      {/if}
      <i class="bi bi-chevron-down tool-selector-chevron" class:open={selectorOpen}></i>
    </button>
    {#if selectorOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="tool-selector-backdrop" onclick={() => selectorOpen = false}></div>
      <div class="tool-selector-dropdown">
        {#each searchTools as group}
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

  {#if subTab === 'text'}
    <!-- Text Search -->
    <div class="search-form">
      <div class="search-input-row">
        <input
          class="search-input"
          bind:this={textSearchInput}
          bind:value={textQuery}
          onkeydown={handleTextKeydown}
          placeholder="Search manuscript..."
        />
        <button class="search-go-btn" onclick={runTextSearch} disabled={!textQuery.trim() || textSearching}>
          <i class="bi bi-search"></i>
        </button>
      </div>

      <div class="search-modes">
        <button class="mode-btn" class:active={searchMode === 'normal'} onclick={() => searchMode = 'normal'}>
          Standard
        </button>
        <button class="mode-btn" class:active={searchMode === 'case'} onclick={() => searchMode = 'case'}>
          Aa
        </button>
        <button class="mode-btn" class:active={searchMode === 'whole'} onclick={() => searchMode = 'whole'}>
          [w] Word
        </button>
        <button class="mode-btn" class:active={searchMode === 'regex'} onclick={() => searchMode = 'regex'}>
          .* Regex
        </button>
        <button class="mode-btn" class:active={searchMode === 'fuzzy'} onclick={() => searchMode = 'fuzzy'}>
          ~ Fuzzy
        </button>
      </div>

      {#if searchMode === 'regex'}
        <div class="search-helper">
          <div class="helper-title"><i class="bi bi-info-circle"></i> Pattern Search</div>
          <p>Find variations of a word or phrase in one search. Use <code>|</code> to mean <em>"or"</em> and parentheses to group options.</p>
          <div class="helper-examples">
            <code>walked|walking|walks</code> <span>any of these words</span>
            <code>walk(ed|ing|s)</code> <span>same thing, shorter</span>
            <code>the (old|new|broken) door</code> <span>"the old door", "the new door", etc.</span>
            <code>colo(u)?r</code> <span>"color" or "colour"</span>
            <code>(Sarah|Mary) said</code> <span>either character speaking</span>
            <code>look(ed)? (at|toward)</code> <span>"look at", "looked toward", etc.</span>
          </div>
        </div>
      {/if}

      {#if searchMode === 'fuzzy'}
        <div class="search-helper">
          <div class="helper-title"><i class="bi bi-info-circle"></i> Fuzzy Search</div>
          <p>Finds words that are <em>close</em> to what you typed, even with slight differences. Great for catching:</p>
          <ul class="helper-list">
            <li><strong>Typos</strong> — "Sarha" still finds "Sarah"</li>
            <li><strong>Spelling variations</strong> — "grey" finds "gray"</li>
            <li><strong>Missing letters</strong> — "hapened" finds "happened"</li>
          </ul>
          <p class="helper-note">Allows one character to be different, added, or missing per word.</p>
        </div>
      {/if}

      <div class="pos-hint">
        <div class="pos-hint-title"><i class="bi bi-lightbulb"></i> Word types</div>
        <p>Use these tags to match any word of a type:</p>
        <div class="helper-examples">
          <code>{'{'}verb{'}'}</code> <span>any action — ran, whispered, grabbed...</span>
          <code>{'{'}adjective{'}'}</code> <span>any describing word — dark, beautiful, old...</span>
          <code>{'{'}adverb{'}'}</code> <span>any -ly word — quickly, softly, nervously...</span>
        </div>
        <div class="pos-examples">
          <span class="pos-eg-label">Try:</span>
          <code>she {'{'}verb{'}'} the door</code>
          <code>{'{'}adj{'}'} hair</code>
          <code>{'{'}adv{'}'} whispered</code>
        </div>
      </div>
    </div>

    <div class="search-results">
      {#if textSearching}
        <div class="search-empty">Searching...</div>
      {:else if textResults && textResults.total_matches === 0}
        <div class="search-empty">No results found.</div>
      {:else if textResults}
        <div class="search-result-count">
          {textResults.total_matches} match{textResults.total_matches !== 1 ? 'es' : ''}
          across {textGrouped().length} chapter{textGrouped().length !== 1 ? 's' : ''}
        </div>
        {#each textGrouped() as chapter (chapter.chapterId)}
          <div class="search-chapter">
            <div class="search-chapter-header">
              <span class="search-chapter-title">{chapter.title}</span>
              <span class="search-chapter-count">{chapter.matchCount}</span>
            </div>
            {#each chapter.results as result}
              <button
                class="search-slab"
                onclick={() => ongotochapter?.(result.chapter_id, result.matched_text, result.char_position)}
                title="Jump to this match"
              >
                <span class="slab-text">
                  &ldquo;...{@html highlightTextMatch(result.context, result.matched_text)}...&rdquo;
                </span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

  {:else if subTab === 'dialogue'}
    <!-- Dialogue Search -->
    <div class="search-form">
      <div class="search-input-row">
        <input
          class="search-input"
          bind:value={dlgQuery}
          onkeydown={handleDlgKeydown}
          placeholder="Search within dialogue..."
        />
        <button class="search-go-btn" onclick={runDialogueSearch} disabled={!dlgQuery.trim() || dlgSearching}>
          <i class="bi bi-search"></i>
        </button>
      </div>

      <div class="search-modes">
        <button class="mode-btn" class:active={dlgMode === 'normal'} onclick={() => dlgMode = 'normal'}>Standard</button>
        <button class="mode-btn" class:active={dlgMode === 'case'} onclick={() => dlgMode = 'case'}>Aa</button>
        <button class="mode-btn" class:active={dlgMode === 'whole'} onclick={() => dlgMode = 'whole'}>[w] Word</button>
        <button class="mode-btn" class:active={dlgMode === 'regex'} onclick={() => dlgMode = 'regex'}>.* Regex</button>
        <button class="mode-btn" class:active={dlgMode === 'fuzzy'} onclick={() => dlgMode = 'fuzzy'}>~ Fuzzy</button>
      </div>

      {#if dlgMode === 'regex'}
        <div class="search-helper">
          <div class="helper-title"><i class="bi bi-info-circle"></i> Pattern Search</div>
          <p>Use <code>|</code> to mean <em>"or"</em> and parentheses to group options.</p>
          <div class="helper-examples">
            <code>love(d|s)?</code> <span>love, loved, loves</span>
            <code>(sorry|forgive)</code> <span>either word in dialogue</span>
          </div>
        </div>
      {/if}

      {#if dlgMode === 'fuzzy'}
        <div class="search-helper">
          <div class="helper-title"><i class="bi bi-info-circle"></i> Fuzzy Search</div>
          <p>Finds close matches — catches typos and spelling variations in dialogue.</p>
        </div>
      {/if}

      <div class="dialogue-info">
        <i class="bi bi-chat-quote"></i>
        Only searches within quoted speech — everything between quotation marks.
      </div>

      <div class="pos-hint">
        <div class="pos-hint-title"><i class="bi bi-lightbulb"></i> Word types</div>
        <p>Use tags to match types of words:</p>
        <div class="pos-examples">
          <code>{'{'}verb{'}'} me</code>
          <code>{'{'}adv{'}'} said</code>
          <code>{'{'}adj{'}'} eyes</code>
        </div>
      </div>
    </div>

    <div class="search-results">
      {#if dlgSearching}
        <div class="search-empty">Searching dialogue...</div>
      {:else if dlgResults && Array.isArray(dlgResults) && dlgResults.length === 0}
        <div class="search-empty">No dialogue matches found.</div>
      {:else if dlgResults && Array.isArray(dlgResults)}
        <div class="search-result-count">
          {dlgResults.length} match{dlgResults.length !== 1 ? 'es' : ''} in dialogue
        </div>
        {#each dlgGrouped() as chapter (chapter.chapterId)}
          <div class="search-chapter">
            <div class="search-chapter-header">
              <span class="search-chapter-title">{chapter.title}</span>
              <span class="search-chapter-count">{chapter.results.length}</span>
            </div>
            {#each chapter.results as result}
              <button
                class="search-slab"
                onclick={() => ongotochapter?.(result.chapter_id, result.dialogue, result.char_position)}
                title="Jump to this dialogue"
              >
                <span class="slab-text">
                  {@html highlightDialogue(result.context, result.matched_text, result.dialogue)}
                </span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

  {:else if subTab === 'relationship'}
    <!-- Relationship Search -->
    <div class="search-form">
      <label class="search-label">
        Entity A
        <select class="search-select" bind:value={entityAId}>
          <option value={null}>Select entity...</option>
          {#each entities as entity (entity.id)}
            <option value={entity.id}>{entity.name}</option>
          {/each}
        </select>
      </label>

      <div class="search-type-row">
        <button class="search-type-btn" class:active={searchType === 'near'} onclick={() => searchType = 'near'}>
          <i class="bi bi-arrows-expand"></i> Appears Near
        </button>
        <button class="search-type-btn" class:active={searchType === 'without'} onclick={() => searchType = 'without'}>
          <i class="bi bi-slash-circle"></i> Without
        </button>
      </div>

      <label class="search-label">
        Entity B
        <select class="search-select" bind:value={entityBId}>
          <option value={null}>Select entity...</option>
          {#each entities as entity (entity.id)}
            <option value={entity.id}>{entity.name}</option>
          {/each}
        </select>
      </label>

      <label class="search-label">
        Max Distance
        <div class="distance-row">
          <input type="range" class="distance-slider" bind:value={maxDistance} min="100" max="5000" step="100" />
          <span class="distance-value">{maxDistance.toLocaleString()} chars</span>
        </div>
      </label>

      <button class="search-run-btn" onclick={runRelSearch} disabled={!entityAId || !entityBId || relSearching}>
        {#if relSearching}
          Searching...
        {:else}
          <i class="bi bi-search"></i> Search
        {/if}
      </button>
    </div>

    <div class="search-results">
      {#if relSearching}
        <div class="search-empty">Scanning manuscript...</div>
      {:else if relHasSearched && relResults.length === 0}
        <div class="search-empty">No results found.</div>
      {:else if relHasSearched}
        <div class="search-result-count">{relResults.length} result{relResults.length !== 1 ? 's' : ''}</div>
        {#each relGrouped() as chapter (chapter.chapterId)}
          <div class="search-chapter">
            <div class="search-chapter-header">
              <span class="search-chapter-title">{chapter.title}</span>
              <span class="search-chapter-count">{chapter.results.length}</span>
            </div>
            {#each chapter.results as result, ri}
              {@const middleKey = `${chapter.chapterId}-${ri}`}
              <div class="search-slab rel-slab">
                <button class="slab-jump" onclick={() => ongotochapter?.(result.chapter_id, result.entity_a_match, result.char_position)} title="Jump">
                  <i class="bi bi-box-arrow-up-right" style="font-size: 0.65rem;"></i>
                </button>
                {#if result.distance > 0}
                  <span class="slab-distance">{result.distance} chars apart</span>
                {/if}
                <div class="slab-text">
                  <span>&ldquo;...{@html highlightEntities(result.lead_in)}</span>
                  {#if result.middle}
                    {#if expandedMiddles.has(middleKey)}
                      <span class="slab-middle-expanded">{@html highlightEntities(result.middle)}</span>
                      <button class="slab-collapse" onclick={() => toggleMiddle(middleKey)}>collapse</button>
                    {:else}
                      <button class="slab-middle-collapsed" onclick={() => toggleMiddle(middleKey)}>
                        ~~ {result.middle.length.toLocaleString()} chars ~~
                      </button>
                    {/if}
                  {/if}
                  <span>{@html highlightEntities(result.lead_out)}...&rdquo;</span>
                </div>
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

  {:else if subTab === 'semantic'}
    <!-- Meaning Search -->
    <div class="search-form">
      <div class="search-input-row">
        <input
          class="search-input"
          type="text"
          placeholder="Describe what you're looking for..."
          bind:value={semQuery}
          onkeydown={e => { if (e.key === 'Enter') runMeaningSearch(); }}
        />
        <button class="search-go-btn" onclick={runMeaningSearch} disabled={!semQuery.trim() || semSearching}>
          {#if semSearching}
            <i class="bi bi-hourglass-split"></i>
          {:else}
            <i class="bi bi-search"></i>
          {/if}
        </button>
      </div>
      <div class="search-modes">
        <button class="mode-btn" class:active={semGranularity === 'sentence'} onclick={() => semGranularity = 'sentence'}>
          Sentences
        </button>
        <button class="mode-btn" class:active={semGranularity === 'paragraph'} onclick={() => semGranularity = 'paragraph'}>
          Paragraphs
        </button>
      </div>
      <div class="sem-status">
        {#if semProgress}
          <span class="sem-status-text">
            <i class="bi bi-hourglass-split"></i>
            Indexing chapter {semProgress.done}/{semProgress.total}...
          </span>
        {:else if semIndexStatus}
          <span class="sem-status-text">
            {semIndexStatus.indexed_chapters}/{semIndexStatus.total_chapters} chapters indexed
            &middot; {semIndexStatus.total_embeddings.toLocaleString()} embeddings
            {#if semIndexStatus.pending > 0}
              &middot; <strong>{semIndexStatus.pending} pending</strong>
            {/if}
          </span>
        {/if}
        <button class="sem-rebuild-btn" onclick={handleRebuildIndex} disabled={semRebuilding || !!semProgress}>
          <i class="bi bi-arrow-clockwise"></i>
          {semRebuilding || semProgress ? 'Indexing...' : 'Rebuild Index'}
        </button>
      </div>

      {#if semResults && semResults.length > 0}
        <div class="sem-threshold">
          <label class="sem-threshold-label">
            Min relevance: <strong>{semThreshold}%</strong>
          </label>
          <input type="range" class="sem-threshold-slider" min="0" max="80" step="5" bind:value={semThreshold} />
          <span class="sem-threshold-count">{semFiltered.length} result{semFiltered.length !== 1 ? 's' : ''}</span>
        </div>
      {/if}
    </div>

    <div class="search-results">
      {#if semSearching}
        <div class="search-empty">Searching...</div>
      {:else if semResults === null}
        <div class="search-empty">
          <p>Describe a scene, emotion, or concept in your own words and find matching passages in your manuscript.</p>
          <p class="search-empty-hint">Works best for finding scenes, themes, and descriptions — e.g. "the moment of betrayal", "descriptions of the house", "characters feeling afraid". For exact words or names, use Text Search instead.</p>
          <p class="search-empty-hint">The index builds automatically as you write. Click "Rebuild Index" to re-index everything.</p>
        </div>
      {:else if semFiltered.length === 0}
        <div class="search-empty">No matches above {semThreshold}% relevance.</div>
      {:else}
        <div class="search-result-count">
          {semFiltered.length} match{semFiltered.length !== 1 ? 'es' : ''}
          across {semGrouped().length} chapter{semGrouped().length !== 1 ? 's' : ''}
        </div>
        {#each semGrouped() as chapter (chapter.chapterId)}
          <div class="search-chapter">
            <div class="search-chapter-header">
              <span class="search-chapter-title">{chapter.title}</span>
              <span class="search-chapter-count">{chapter.results.length}</span>
            </div>
            {#each chapter.results as result}
              <button
                class="search-slab"
                onclick={() => ongotochapter?.(result.chapter_id, result.segment_text.slice(0, 40), result.char_start)}
                title="Jump to this match"
              >
                <span class="slab-text">
                  &ldquo;{result.segment_text.length > 150 ? result.segment_text.slice(0, 150) + '...' : result.segment_text}&rdquo;
                </span>
                <span class="sem-score">{Math.round(result.score * 100)}%</span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .search-panel {
    display: flex; flex-direction: column; height: 100%;
    font-family: var(--iwe-font-ui);
  }

  /* Tool selector dropdown */
  .tool-selector-wrap {
    position: relative; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
  }
  .tool-selector-btn {
    display: flex; align-items: center; gap: 0.4rem;
    width: 100%; padding: 0.5rem 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem; font-weight: 500;
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
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    background: none; border: none; color: var(--iwe-text-secondary);
    cursor: pointer; text-align: left; transition: all 100ms;
  }
  .tool-option:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .tool-option.active { color: var(--iwe-accent); font-weight: 500; }
  .tool-option-check { margin-left: auto; font-size: 0.75rem; }

  /* Form */
  .search-form {
    padding: 0.6rem 0.75rem;
    display: flex; flex-direction: column; gap: 0.5rem;
    border-bottom: 1px solid var(--iwe-border-light);
  }

  .search-input-row {
    display: flex; gap: 0.3rem;
  }
  .search-input {
    flex: 1; font-family: var(--iwe-font-ui); font-size: 0.9rem;
    padding: 0.4rem 0.6rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none;
  }
  .search-input:focus { border-color: var(--iwe-accent); }
  .search-input::placeholder { color: var(--iwe-text-faint); }
  .search-go-btn {
    background: var(--iwe-accent); color: white; border: none;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    padding: 0.4rem 0.6rem; transition: all 150ms;
  }
  .search-go-btn:hover:not(:disabled) { background: var(--iwe-accent-hover); }
  .search-go-btn:disabled { opacity: 0.4; }

  .search-modes {
    display: flex; gap: 0.2rem; flex-wrap: wrap;
  }
  .mode-btn {
    font-family: var(--iwe-font-ui); font-size: 0.7rem; font-weight: 500;
    padding: 0.25rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted); transition: all 100ms;
  }
  .mode-btn:hover { border-color: var(--iwe-accent); color: var(--iwe-text-secondary); }
  .mode-btn.active {
    background: var(--iwe-accent-light); color: var(--iwe-accent);
    border-color: var(--iwe-accent);
  }

  .search-label {
    display: flex; flex-direction: column; gap: 0.2rem;
    font-size: 0.7rem; font-weight: 600; color: var(--iwe-text-muted);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .search-select {
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    padding: 0.35rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); background: var(--iwe-bg);
    color: var(--iwe-text); outline: none;
  }
  .search-select:focus { border-color: var(--iwe-accent); }

  .search-type-row { display: flex; gap: 0.3rem; }
  .search-type-btn {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500;
    padding: 0.35rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-secondary); transition: all 100ms;
  }
  .search-type-btn:hover { border-color: var(--iwe-accent); }
  .search-type-btn.active {
    background: var(--iwe-accent-light); color: var(--iwe-accent); border-color: var(--iwe-accent);
  }

  .distance-row { display: flex; align-items: center; gap: 0.5rem; }
  .distance-slider { flex: 1; accent-color: var(--iwe-accent); }
  .distance-value {
    font-size: 0.75rem; color: var(--iwe-text-secondary);
    white-space: nowrap; min-width: 70px; text-align: right;
  }

  .search-run-btn {
    font-family: var(--iwe-font-ui); font-size: 0.9rem; font-weight: 500;
    padding: 0.45rem 0.75rem; border: none;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: var(--iwe-accent); color: white;
    display: flex; align-items: center; justify-content: center; gap: 0.35rem;
    transition: all 150ms;
  }
  .search-run-btn:hover:not(:disabled) { background: var(--iwe-accent-hover); }
  .search-run-btn:disabled { opacity: 0.4; cursor: default; }

  /* Results */
  .search-results { flex: 1; overflow-y: auto; }
  .search-empty {
    padding: 2rem 1rem; text-align: center;
    font-size: 0.9rem; color: var(--iwe-text-faint); font-style: italic;
  }
  .search-result-count {
    padding: 0.4rem 0.75rem; font-size: 0.7rem; color: var(--iwe-text-faint);
    font-weight: 500; border-bottom: 1px solid var(--iwe-border-light);
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
    transition: background 100ms;
  }
  .search-slab:last-child { border-bottom: none; }
  .search-slab:hover { background: var(--iwe-bg-hover); }

  .rel-slab { position: relative; cursor: default; }
  .slab-jump {
    position: absolute; top: 0.5rem; right: 0.5rem;
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text-faint); padding: 0.2rem 0.35rem;
    transition: all 100ms;
  }
  .slab-jump:hover { color: var(--iwe-accent); border-color: var(--iwe-accent); }

  .slab-distance {
    display: block; font-size: 0.65rem; color: var(--iwe-text-faint);
    margin-bottom: 0.25rem;
  }
  .slab-text {
    font-family: var(--iwe-font-prose); font-size: 0.9rem;
    color: var(--iwe-text-secondary); line-height: 1.7;
  }

  .slab-middle-collapsed {
    display: inline;
    font-family: var(--iwe-font-ui); font-size: 0.7rem; font-weight: 500;
    color: var(--iwe-accent); background: var(--iwe-accent-light);
    border: 1px solid var(--iwe-accent); border-radius: 10px;
    padding: 0.1rem 0.5rem; margin: 0 0.25rem;
    cursor: pointer; transition: all 100ms;
  }
  .slab-middle-collapsed:hover { background: var(--iwe-accent); color: white; }
  .slab-middle-expanded { color: var(--iwe-text-muted); }
  .slab-collapse {
    display: inline;
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-faint); background: none;
    border: 1px solid var(--iwe-border); border-radius: 10px;
    padding: 0.05rem 0.4rem; margin: 0 0.25rem; cursor: pointer;
  }
  .slab-collapse:hover { border-color: var(--iwe-accent); color: var(--iwe-accent); }

  /* Helper boxes */
  .search-helper {
    background: var(--iwe-accent-light); border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius-sm); padding: 0.6rem 0.75rem;
    font-size: 0.85rem; color: var(--iwe-text-secondary); line-height: 1.55;
  }
  .helper-title {
    font-weight: 600; color: var(--iwe-accent); margin-bottom: 0.3rem;
    display: flex; align-items: center; gap: 0.35rem; font-size: 0.85rem;
  }
  .search-helper p { margin: 0 0 0.3rem; }
  .helper-examples {
    display: grid; grid-template-columns: auto 1fr; gap: 0.2rem 0.55rem;
    margin-top: 0.25rem;
  }
  .helper-examples code {
    font-family: monospace; font-size: 0.85rem; font-weight: 600;
    color: var(--iwe-accent); background: var(--iwe-bg);
    padding: 0.1rem 0.3rem; border-radius: 2px;
  }
  .helper-examples span { font-size: 0.85rem; color: var(--iwe-text-faint); }
  .helper-list {
    margin: 0.25rem 0 0.35rem; padding-left: 1.2rem;
    font-size: 0.85rem;
  }
  .helper-list li { margin-bottom: 0.2rem; }
  .helper-note {
    font-size: 0.8rem; color: var(--iwe-text-faint); font-style: italic;
    margin: 0;
  }

  /* POS tag hint */
  .pos-hint {
    background: var(--iwe-bg-hover); border: 1px solid var(--iwe-border-light);
    border-radius: var(--iwe-radius-sm); padding: 0.6rem 0.75rem;
    font-size: 0.85rem; color: var(--iwe-text-secondary); line-height: 1.55;
  }
  .pos-hint p { margin: 0 0 0.3rem; }
  .pos-hint-title {
    font-weight: 600; color: var(--iwe-text-muted); margin-bottom: 0.3rem;
    display: flex; align-items: center; gap: 0.35rem; font-size: 0.85rem;
  }
  .pos-examples {
    display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: 0.3rem;
  }
  .pos-examples code {
    font-family: monospace; font-size: 0.85rem;
    background: var(--iwe-bg); padding: 0.15rem 0.4rem;
    border-radius: 3px; border: 1px solid var(--iwe-border-light);
    color: var(--iwe-text-secondary);
  }
  .pos-eg-label { font-size: 0.8rem; color: var(--iwe-text-faint); align-self: center; }

  .dialogue-info {
    font-size: 0.7rem; color: var(--iwe-text-faint);
    display: flex; align-items: center; gap: 0.35rem;
    padding: 0.3rem 0; font-style: italic;
  }

  :global(.search-highlight) {
    background: #fde68a; color: var(--iwe-text); font-weight: 600;
    border-radius: 2px; padding: 0 2px;
  }

  /* Semantic search */
  .sem-threshold {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.3rem 0.6rem;
    border-top: 1px solid var(--iwe-border-light);
  }
  .sem-threshold-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); white-space: nowrap;
  }
  .sem-threshold-label strong { color: var(--iwe-accent); }
  .sem-threshold-slider {
    flex: 1; height: 4px; accent-color: var(--iwe-accent, #2d6a5e);
  }
  .sem-threshold-count {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    color: var(--iwe-text-faint); white-space: nowrap;
  }
  .sem-status {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    border-top: 1px solid var(--iwe-border-light);
  }
  .sem-status-text {
    font-size: 0.7rem; color: var(--iwe-text-faint);
    flex: 1;
  }
  .sem-rebuild-btn {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    padding: 0.2rem 0.5rem; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    background: none; color: var(--iwe-text-muted);
    display: flex; align-items: center; gap: 0.25rem;
    transition: all 100ms; white-space: nowrap;
  }
  .sem-rebuild-btn:hover:not(:disabled) { border-color: var(--iwe-accent); color: var(--iwe-accent); }
  .sem-rebuild-btn:disabled { opacity: 0.5; cursor: default; }
  .sem-score {
    font-family: var(--iwe-font-ui); font-size: 0.65rem;
    font-weight: 600; color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.08);
    padding: 0.1rem 0.35rem; border-radius: 3px;
    flex-shrink: 0;
  }
</style>
