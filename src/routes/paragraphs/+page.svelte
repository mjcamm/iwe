<script>
  import { onMount } from 'svelte';
  import { paragraphLengthAnalysis } from '$lib/db.js';
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let data = $state(null);
  let loading = $state(true);
  let tooltip = $state(null);

  onMount(async () => {
    try {
      data = await paragraphLengthAnalysis();
    } catch (e) {
      console.warn('Paragraph length analysis failed:', e);
    }
    loading = false;
  });

  function variationLabel(pct) {
    if (pct < 25) return 'Very uniform';
    if (pct < 45) return 'Low variation';
    if (pct < 70) return 'Moderate variation';
    if (pct < 100) return 'Good variation';
    return 'High variation';
  }

  function barHeight(wordCount, maxWords) {
    if (maxWords === 0) return 0;
    return Math.max(3, (wordCount / maxWords) * 100);
  }

  function showTooltip(e, para, index) {
    const rect = e.currentTarget.getBoundingClientRect();
    tooltip = {
      x: rect.left + rect.width / 2,
      y: rect.top - 8,
      text: `#${index + 1} — ${para.word_count} words`,
      preview: para.preview.length > 60 ? para.preview.slice(0, 60) + '...' : para.preview,
    };
  }

  function hideTooltip() {
    tooltip = null;
  }

  async function goToParagraph(chapterId, preview, charStart) {
    const searchText = preview.length > 60 ? preview.slice(0, 60) : preview;
    try {
      await emitTo('main', 'navigate-to-position', {
        chapterId,
        searchText,
        charPosition: charStart,
      });
      await getCurrentWindow().minimize();
    } catch (err) {
      console.warn('Failed to emit navigation event:', err);
    }
  }
</script>

<div class="pl-page">
  <header class="pl-header">
    <h1>Paragraph Length</h1>
    <p class="pl-desc">The visual texture of your page matters. Uniform paragraph lengths create monotony — a mix of short punchy paragraphs and longer flowing ones creates rhythm. Highlighted bars indicate runs of 3+ paragraphs with similar length.</p>
  </header>

  {#if loading}
    <div class="pl-loading">Analysing paragraphs...</div>
  {:else if data && data.chapters.length > 0}
    <div class="pl-content">
      {#each data.chapters as ch (ch.chapter_id)}
        {@const maxWords = Math.max(...ch.paragraphs.map(p => p.word_count), 1)}
        <div class="chapter-section">
          <div class="chapter-heading">
            <h2 class="chapter-title">{ch.chapter_title}</h2>
            <div class="chapter-stats">
              <span class="stat">{ch.total_paragraphs} paragraphs</span>
              <span class="stat">avg <strong>{ch.avg_length}</strong> words</span>
              <span class="stat">variation <strong>{ch.variation_pct}%</strong></span>
              <span class="stat-label">{variationLabel(ch.variation_pct)}</span>
            </div>
          </div>
          <div class="col-chart-wrap">
            <div class="col-chart" style="min-width: {Math.max(ch.paragraphs.length * 18, 300)}px">
              {#each ch.paragraphs as para, i}
                <div
                  class="col-slot"
                  onclick={() => goToParagraph(ch.chapter_id, para.preview, para.char_start)}
                  onmouseenter={(e) => showTooltip(e, para, i)}
                  onmouseleave={hideTooltip}
                >
                  <div class="col-bar-area">
                    <div
                      class="col-bar"
                      class:mono-bar={para.monotonous}
                      style="height: {barHeight(para.word_count, maxWords)}%"
                    ></div>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="pl-loading">No chapters found.</div>
  {/if}

  {#if tooltip}
    <div class="tooltip" style="left: {tooltip.x}px; top: {tooltip.y}px">
      <div class="tooltip-title">{tooltip.text}</div>
      <div class="tooltip-preview">{tooltip.preview}</div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }

  .pl-page {
    display: flex; flex-direction: column; min-height: 100vh;
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #fffef9; color: #2d2a26;
  }

  .pl-header {
    padding: 1.5rem 2rem 1rem;
    border-bottom: 1px solid #e5e1da;
    background: #faf8f5;
  }
  .pl-header h1 {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.4rem; font-weight: 400; margin: 0 0 0.3rem;
  }
  .pl-desc {
    font-size: 0.8rem; color: #9e9891; margin: 0; line-height: 1.5;
    max-width: 700px;
  }

  .pl-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic;
  }

  .pl-content { padding: 1.5rem 2rem 2rem; }

  .chapter-section { margin-bottom: 2.5rem; }
  .chapter-heading {
    display: flex; align-items: baseline; gap: 1rem; margin-bottom: 0.6rem;
    flex-wrap: wrap;
  }
  .chapter-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 400; margin: 0; color: #2d2a26;
  }
  .chapter-stats {
    display: flex; gap: 0.8rem; align-items: baseline;
    font-size: 0.8rem; color: #9e9891;
  }
  .chapter-stats strong { color: #6b6560; }
  .stat-label { font-style: italic; }

  .col-chart-wrap {
    border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem 0.8rem;
    overflow-x: auto;
  }

  .col-chart {
    display: flex; align-items: flex-end; gap: 3px;
    height: 160px;
  }

  .col-slot {
    flex: 1; min-width: 10px; max-width: 40px;
    height: 100%;
    display: flex; flex-direction: column; justify-content: flex-end;
    cursor: pointer;
    border-radius: 2px 2px 0 0;
    transition: background 0.15s;
  }
  .col-slot:hover { background: rgba(45, 106, 94, 0.06); }

  .col-bar-area {
    width: 100%; height: 100%;
    display: flex; align-items: flex-end;
  }

  .col-bar {
    width: 100%; background: #2d6a5e;
    border-radius: 2px 2px 0 0;
    transition: height 0.3s ease;
  }
  .col-bar.mono-bar {
    background: #d4a574;
  }

  .tooltip {
    position: fixed; transform: translate(-50%, -100%);
    background: rgba(45, 42, 38, 0.92); color: white;
    padding: 6px 10px; border-radius: 5px;
    pointer-events: none; z-index: 1000;
    max-width: 320px;
  }
  .tooltip-title {
    font-size: 0.8rem; font-weight: 600; white-space: nowrap;
  }
  .tooltip-preview {
    font-size: 0.72rem; color: #ccc; margin-top: 2px;
    line-height: 1.3;
  }
</style>
