<script>
  import { onMount } from 'svelte';
  import { generateHeatmap } from '$lib/db.js';

  let data = $state(null);
  let loading = $state(true);
  let viewMode = $state('chapter'); // 'chapter' | 'sentence'
  let chapterCanvas;
  let sentenceCanvas;
  let hoveredCell = $state(null);

  onMount(async () => {
    // Get entity IDs from URL params
    const params = new URLSearchParams(window.location.search);
    const ids = params.get('entities')?.split(',').map(Number).filter(n => !isNaN(n)) || [];

    if (ids.length === 0) {
      loading = false;
      return;
    }

    try {
      data = await generateHeatmap(ids);
    } catch (e) {
      console.warn('Heatmap generation failed:', e);
    }
    loading = false;
  });

  $effect(() => {
    if (data && viewMode === 'chapter' && chapterCanvas) {
      drawChapterHeatmap();
    }
  });

  $effect(() => {
    if (data && viewMode === 'sentence' && sentenceCanvas) {
      drawSentenceHeatmap();
    }
  });

  function drawChapterHeatmap() {
    const canvas = chapterCanvas;
    const ctx = canvas.getContext('2d');
    if (!data || !ctx) return;

    const entities = data.entities;
    const chapters = data.chapters;
    const grid = data.chapter_grid;

    const labelWidth = 150;
    const headerHeight = 80;
    const cellW = Math.max(40, Math.min(80, (canvas.parentElement.clientWidth - labelWidth - 40) / chapters.length));
    const cellH = 36;

    canvas.width = labelWidth + chapters.length * cellW + 20;
    canvas.height = headerHeight + entities.length * cellH + 20;

    const dpr = window.devicePixelRatio || 1;
    canvas.width *= dpr;
    canvas.height *= dpr;
    canvas.style.width = `${canvas.width / dpr}px`;
    canvas.style.height = `${canvas.height / dpr}px`;
    ctx.scale(dpr, dpr);

    const w = canvas.width / dpr;
    const h = canvas.height / dpr;

    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, w, h);

    // Find max for color scaling
    let maxCount = 1;
    for (const row of grid) {
      for (const val of row) {
        if (val > maxCount) maxCount = val;
      }
    }

    // Chapter headers (rotated)
    ctx.save();
    ctx.font = '11px Source Sans 3, system-ui, sans-serif';
    ctx.fillStyle = '#6b6560';
    for (let j = 0; j < chapters.length; j++) {
      const x = labelWidth + j * cellW + cellW / 2;
      ctx.save();
      ctx.translate(x, headerHeight - 8);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = 'left';
      ctx.fillText(chapters[j].title, 0, 0);
      ctx.restore();
    }
    ctx.restore();

    // Entity labels and grid
    for (let i = 0; i < entities.length; i++) {
      const y = headerHeight + i * cellH;
      const entity = entities[i];

      // Label
      ctx.font = '12px Source Sans 3, system-ui, sans-serif';
      ctx.fillStyle = entity.color;
      ctx.textAlign = 'right';
      ctx.textBaseline = 'middle';
      ctx.fillText(entity.name, labelWidth - 10, y + cellH / 2);

      // Cells
      for (let j = 0; j < chapters.length; j++) {
        const x = labelWidth + j * cellW;
        const count = grid[i][j];
        const intensity = count / maxCount;

        // Parse entity color
        const color = hexToRgb(entity.color);
        if (count === 0) {
          ctx.fillStyle = '#f0ede8';
        } else {
          const alpha = 0.1 + intensity * 0.85;
          ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${alpha})`;
        }

        ctx.fillRect(x + 1, y + 1, cellW - 2, cellH - 2);

        // Count label
        if (count > 0) {
          ctx.font = '10px Source Sans 3, system-ui, sans-serif';
          ctx.fillStyle = intensity > 0.5 ? '#fff' : '#6b6560';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillText(count.toString(), x + cellW / 2, y + cellH / 2);
        }
      }
    }
  }

  function drawSentenceHeatmap() {
    const canvas = sentenceCanvas;
    const ctx = canvas.getContext('2d');
    if (!data || !ctx) return;

    const entities = data.entities;
    const grid = data.sentence_grid;
    const totalSentences = data.total_sentences;
    const breaks = data.sentence_chapter_breaks;

    if (totalSentences === 0) return;

    const labelWidth = 150;
    const rowHeight = 40;
    const headerHeight = 30;
    const pixelsPerSentence = Math.max(1, Math.min(4, (canvas.parentElement.clientWidth - labelWidth - 40) / totalSentences));

    const dpr = window.devicePixelRatio || 1;
    const logicalW = labelWidth + totalSentences * pixelsPerSentence + 20;
    const logicalH = headerHeight + entities.length * rowHeight + 20;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = `${logicalW}px`;
    canvas.style.height = `${logicalH}px`;
    ctx.scale(dpr, dpr);

    // Background
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Chapter break lines
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 0.5;
    for (const brk of breaks) {
      const x = labelWidth + brk * pixelsPerSentence;
      ctx.beginPath();
      ctx.moveTo(x, headerHeight);
      ctx.lineTo(x, logicalH - 10);
      ctx.stroke();
    }

    // Chapter labels at top
    ctx.font = '9px Source Sans 3, system-ui, sans-serif';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'left';
    for (let i = 0; i < breaks.length; i++) {
      const x = labelWidth + breaks[i] * pixelsPerSentence + 3;
      const title = data.chapters[i]?.title || '';
      ctx.fillText(title, x, headerHeight - 6);
    }

    // Entity rows
    for (let i = 0; i < entities.length; i++) {
      const entity = entities[i];
      const y = headerHeight + i * rowHeight;
      const row = grid[i] || [];
      const color = hexToRgb(entity.color);

      // Label
      ctx.font = '11px Source Sans 3, system-ui, sans-serif';
      ctx.fillStyle = entity.color;
      ctx.textAlign = 'right';
      ctx.textBaseline = 'middle';
      ctx.fillText(entity.name, labelWidth - 8, y + rowHeight / 2);

      // Background strip
      ctx.fillStyle = '#f5f2ed';
      ctx.fillRect(labelWidth, y + 2, totalSentences * pixelsPerSentence, rowHeight - 4);

      // Draw presence blocks
      for (let s = 0; s < totalSentences; s++) {
        if (row[s] === 1) {
          ctx.fillStyle = `rgb(${color.r}, ${color.g}, ${color.b})`;
          ctx.fillRect(
            labelWidth + s * pixelsPerSentence,
            y + 2,
            Math.max(pixelsPerSentence, 1),
            rowHeight - 4
          );
        }
      }
    }
  }

  function hexToRgb(hex) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
      r: parseInt(result[1], 16),
      g: parseInt(result[2], 16),
      b: parseInt(result[3], 16)
    } : { r: 100, g: 100, b: 100 };
  }
</script>

<div class="heatmap-page">
  <div class="heatmap-toolbar">
    <h1 class="heatmap-title">Entity Presence Heatmap</h1>
    <div class="heatmap-tabs">
      <button class="hm-tab" class:active={viewMode === 'chapter'} onclick={() => viewMode = 'chapter'}>
        <i class="bi bi-grid-3x3"></i> Chapter Grid
      </button>
      <button class="hm-tab" class:active={viewMode === 'sentence'} onclick={() => viewMode = 'sentence'}>
        <i class="bi bi-distribute-horizontal"></i> Sentence Timeline
      </button>
    </div>
  </div>

  {#if loading}
    <div class="hm-loading">Analysing manuscript...</div>
  {:else if !data}
    <div class="hm-loading">No data. Close this window and try again.</div>
  {:else}
    <div class="heatmap-content">
      {#if viewMode === 'chapter'}
        <div class="hm-info">
          Each cell shows how many times an entity is mentioned in that chapter. Darker = more mentions.
        </div>
        <div class="canvas-wrap">
          <canvas bind:this={chapterCanvas}></canvas>
        </div>
      {:else}
        <div class="hm-info">
          Each colored tick is a sentence where the entity appears. Like a DNA sequence — you can see patterns, gaps, and overlaps at a glance.
        </div>
        <div class="canvas-wrap sentence-wrap">
          <canvas bind:this={sentenceCanvas}></canvas>
        </div>
      {/if}

      <div class="hm-legend">
        {#each data.entities as entity}
          <div class="legend-item">
            <span class="legend-dot" style="background: {entity.color}"></span>
            <span class="legend-name">{entity.name}</span>
            <span class="legend-type">{entity.entity_type}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    overflow: auto !important;
    height: auto !important;
  }
  .heatmap-page {
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: var(--iwe-bg-warm, #faf8f5);
    min-height: 100vh;
    display: flex; flex-direction: column;
  }

  .heatmap-toolbar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #e5e1da;
    background: white;
  }
  .heatmap-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.2rem; font-weight: 400; margin: 0;
    color: #2d2a26;
  }
  .heatmap-tabs { display: flex; gap: 0.3rem; }
  .hm-tab {
    font-family: inherit; font-size: 0.8rem; font-weight: 500;
    padding: 0.4rem 0.8rem; border: 1px solid #e5e1da;
    border-radius: 4px; cursor: pointer; background: none;
    color: #6b6560; display: flex; align-items: center; gap: 0.3rem;
    transition: all 150ms;
  }
  .hm-tab:hover { border-color: #2d6a5e; color: #2d6a5e; }
  .hm-tab.active { background: #e8f2ef; color: #2d6a5e; border-color: #2d6a5e; }

  .hm-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic; font-size: 1rem;
  }

  .heatmap-content {
    flex: 1; overflow: auto; padding: 1rem 1.5rem;
  }
  .hm-info {
    font-size: 0.8rem; color: #9e9891; margin-bottom: 1rem;
    font-style: italic;
  }

  .canvas-wrap {
    overflow-x: auto; overflow-y: auto;
    border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem;
  }
  .sentence-wrap {
    max-height: 60vh;
  }

  canvas { display: block; }

  .hm-legend {
    display: flex; flex-wrap: wrap; gap: 0.75rem;
    margin-top: 1rem; padding: 0.75rem;
    background: white; border: 1px solid #e5e1da; border-radius: 6px;
  }
  .legend-item {
    display: flex; align-items: center; gap: 0.35rem;
  }
  .legend-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .legend-name { font-size: 0.85rem; font-weight: 500; color: #2d2a26; }
  .legend-type { font-size: 0.7rem; color: #9e9891; text-transform: capitalize; }
</style>
