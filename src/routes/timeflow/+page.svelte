<script>
  import { onMount } from 'svelte';
  import { getAllTimeSections, getTimeSectionOrder, saveTimeSectionOrder, resetTimeSectionOrder } from '$lib/db.js';
  import { dndzone } from 'svelte-dnd-action';

  let loading = $state(true);
  let cards = $state([]);
  let hasChanges = $state(false);

  onMount(async () => {
    await loadSections();
  });

  async function loadSections() {
    loading = true;
    try {
      const chaptersData = await getAllTimeSections();
      const savedOrder = await getTimeSectionOrder();

      // Build card list: each chapter without time breaks = 1 card,
      // chapters with breaks = multiple cards (one per section)
      let allCards = [];
      for (const ch of chaptersData) {
        for (const sec of ch.sections) {
          const isFlow = sec.is_flow;
          let displayLabel;
          if (sec.section_index === 0 && isFlow) {
            displayLabel = ch.sections.length > 1 ? 'Flow (1)' : 'Flow';
          } else if (isFlow) {
            displayLabel = `Flow (${sec.section_index + 1})`;
          } else {
            displayLabel = sec.label || 'Time break';
          }

          allCards.push({
            id: `${ch.chapter_id}-${sec.section_index}`,
            chapterId: ch.chapter_id,
            chapterTitle: ch.chapter_title,
            sortOrder: ch.sort_order,
            sectionIndex: sec.section_index,
            label: displayLabel,
            isFlow: isFlow,
            previewText: sec.preview_text,
            wordCount: sec.word_count,
            rawLabel: sec.label,
          });
        }
      }

      // Apply saved order if it exists
      if (savedOrder.length > 0) {
        const orderMap = new Map();
        for (const entry of savedOrder) {
          orderMap.set(`${entry.chapter_id}-${entry.section_index}`, entry.story_order);
        }
        allCards.sort((a, b) => {
          const oa = orderMap.get(a.id) ?? (a.sortOrder * 1000 + a.sectionIndex);
          const ob = orderMap.get(b.id) ?? (b.sortOrder * 1000 + b.sectionIndex);
          return oa - ob;
        });
      } else {
        // Default: chapter order, sections in document order
        allCards.sort((a, b) => {
          if (a.sortOrder !== b.sortOrder) return a.sortOrder - b.sortOrder;
          return a.sectionIndex - b.sectionIndex;
        });
      }

      cards = allCards;
      hasChanges = false;
    } catch (e) {
      console.error('[timeflow] load failed:', e);
    }
    loading = false;
  }

  function handleConsider(e) {
    cards = e.detail.items;
  }

  function handleFinalize(e) {
    cards = e.detail.items;
    hasChanges = true;
  }

  async function save() {
    try {
      const entries = cards.map((card, idx) => [
        card.chapterId,
        card.sectionIndex,
        card.rawLabel || '',
        idx,
      ]);
      await saveTimeSectionOrder(entries);
      hasChanges = false;
    } catch (e) {
      console.error('[timeflow] save failed:', e);
    }
  }

  async function reset() {
    try {
      await resetTimeSectionOrder();
      await loadSections();
    } catch (e) {
      console.error('[timeflow] reset failed:', e);
    }
  }

  function truncatePreview(text, maxLen = 120) {
    if (!text || text.length <= maxLen) return text || '';
    return text.slice(0, maxLen).trim() + '...';
  }
</script>

<div class="timeflow-page">
  <div class="timeflow-header">
    <h1 class="timeflow-title">
      <i class="bi bi-clock-history"></i>
      Time Flow
    </h1>
    <p class="timeflow-desc">Drag sections to arrange them in story-time order (earliest first). Chapters without time breaks appear as single cards.</p>
    <div class="timeflow-actions">
      <button class="btn-author" onclick={save} disabled={!hasChanges}>
        <i class="bi bi-check-lg"></i> Save Order
      </button>
      <button class="btn-author btn-author-subtle" onclick={reset}>
        <i class="bi bi-arrow-counterclockwise"></i> Reset to Chapter Order
      </button>
    </div>
  </div>

  {#if loading}
    <div class="timeflow-loading">Loading sections...</div>
  {:else if cards.length === 0}
    <div class="timeflow-empty">No chapters found. Create some chapters first.</div>
  {:else}
    <div class="timeflow-list"
      use:dndzone={{ items: cards, flipDurationMs: 200 }}
      onconsider={handleConsider}
      onfinalize={handleFinalize}>
      {#each cards as card, idx (card.id)}
        <div class="timeflow-card" class:is-shifted={!card.isFlow}>
          <div class="timeflow-card-grip">
            <i class="bi bi-grip-vertical"></i>
          </div>
          <div class="timeflow-card-number">{idx + 1}</div>
          <div class="timeflow-card-body">
            <div class="timeflow-card-header">
              <span class="timeflow-card-chapter">{card.chapterTitle}</span>
              {#if card.isFlow}
                <span class="timeflow-card-label flow-label">{card.label}</span>
              {:else}
                <span class="timeflow-card-label shifted-label">
                  <i class="bi bi-clock-history"></i>
                  {card.label}
                </span>
              {/if}
              <span class="timeflow-card-wc">{card.wordCount} words</span>
            </div>
            {#if card.previewText}
              <div class="timeflow-card-preview">{truncatePreview(card.previewText)}</div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }

  .timeflow-page {
    font-family: 'Source Sans 3', sans-serif;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem 1.5rem;
    color: #3d3a37;
  }
  .timeflow-header {
    margin-bottom: 1.5rem;
  }
  .timeflow-title {
    font-family: 'Libre Baskerville', serif;
    font-size: 1.5rem;
    font-weight: 700;
    color: #2d6a5e;
    margin: 0 0 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .timeflow-desc {
    font-size: 0.85rem;
    color: #7a756f;
    margin: 0 0 1rem;
  }
  .timeflow-actions {
    display: flex;
    gap: 0.5rem;
  }
  .btn-author {
    font-family: 'Source Sans 3', sans-serif;
    font-size: 0.82rem;
    padding: 0.4rem 0.8rem;
    border: 1px solid #2d6a5e;
    border-radius: 6px;
    background: #2d6a5e;
    color: #fff;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .btn-author:disabled { opacity: 0.5; cursor: default; }
  .btn-author-subtle {
    background: transparent;
    color: #2d6a5e;
    border-color: #2d6a5e44;
  }
  .btn-author-subtle:hover { background: rgba(45, 106, 94, 0.06); }
  .timeflow-loading, .timeflow-empty {
    text-align: center;
    padding: 3rem;
    color: #7a756f;
    font-style: italic;
  }

  .timeflow-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .timeflow-card {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    background: #fffef9;
    border: 1px solid #e8e4de;
    border-radius: 8px;
    padding: 0.75rem;
    transition: box-shadow 0.15s, border-color 0.15s;
    cursor: grab;
  }
  .timeflow-card:hover {
    border-color: #c8c3bb;
    box-shadow: 0 2px 8px rgba(0,0,0,0.06);
  }
  .timeflow-card.is-shifted {
    border-left: 3px solid #2d6a5e;
  }
  .timeflow-card-grip {
    color: #c8c3bb;
    font-size: 1rem;
    padding-top: 2px;
    cursor: grab;
  }
  .timeflow-card-number {
    font-size: 0.75rem;
    font-weight: 700;
    color: #7a756f;
    background: #f0ede8;
    border-radius: 50%;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .timeflow-card-body {
    flex: 1;
    min-width: 0;
  }
  .timeflow-card-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.25rem;
    flex-wrap: wrap;
  }
  .timeflow-card-chapter {
    font-weight: 600;
    font-size: 0.85rem;
    color: #3d3a37;
  }
  .timeflow-card-label {
    font-size: 0.72rem;
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
  }
  .flow-label {
    background: #f0ede8;
    color: #7a756f;
  }
  .shifted-label {
    background: rgba(45, 106, 94, 0.1);
    color: #2d6a5e;
    font-style: italic;
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  .timeflow-card-wc {
    margin-left: auto;
    font-size: 0.72rem;
    color: #9e998f;
  }
  .timeflow-card-preview {
    font-family: 'Libre Baskerville', serif;
    font-size: 0.78rem;
    color: #7a756f;
    line-height: 1.5;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
