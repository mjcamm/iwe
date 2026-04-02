<script>
  import { onMount } from 'svelte';
  import { pacingAnalysis } from '$lib/db.js';
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let data = $state(null);
  let loading = $state(true);
  let canvases = $state({});
  let overlays = $state({});
  let smoothing = $state(3); // rolling average window

  // Per-chart metadata for mouse interaction
  let chartMeta = {};

  onMount(async () => {
    try {
      data = await pacingAnalysis();
    } catch (e) {
      console.warn('Pacing analysis failed:', e);
    }
    loading = false;
  });

  $effect(() => {
    if (!data) return;
    for (const ch of data) {
      const canvas = canvases[ch.chapter_id];
      if (canvas) drawWaveform(canvas, ch);
    }
  });

  function smooth(arr, window) {
    if (window <= 1) return arr;
    const half = Math.floor(window / 2);
    return arr.map((_, i) => {
      let sum = 0, count = 0;
      for (let j = Math.max(0, i - half); j <= Math.min(arr.length - 1, i + half); j++) {
        sum += arr[j];
        count++;
      }
      return sum / count;
    });
  }

  function drawWaveform(canvas, chapter) {
    const raw = chapter.sentence_lengths;
    if (!raw || raw.length === 0) return;
    const vals = smooth(raw, smoothing);

    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 700;
    const logicalW = Math.max(containerW, 400);
    const chartH = 140;
    const padTop = 25;
    const padBottom = 30;
    const padLeft = 40;
    const padRight = 15;
    const logicalH = chartH + padTop + padBottom;
    const drawW = logicalW - padLeft - padRight;
    const stepX = drawW / Math.max(raw.length - 1, 1);
    const maxVal = Math.max(...vals, 1);

    // Store geometry for mouse interaction
    chartMeta[chapter.chapter_id] = {
      padLeft, padTop, chartH, drawW, stepX, maxVal, raw, vals,
      chapter, logicalW, logicalH, padRight
    };

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);

    // Size the overlay to match
    const overlay = overlays[chapter.chapter_id];
    if (overlay) {
      overlay.width = logicalW * dpr;
      overlay.height = logicalH * dpr;
      overlay.style.width = logicalW + 'px';
      overlay.style.height = logicalH + 'px';
    }

    // Background
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Y-axis grid lines
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 0.5;
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    const gridSteps = 4;
    for (let i = 0; i <= gridSteps; i++) {
      const y = padTop + (chartH * i / gridSteps);
      const val = Math.round(maxVal * (1 - i / gridSteps));
      ctx.beginPath();
      ctx.moveTo(padLeft, y);
      ctx.lineTo(logicalW - padRight, y);
      ctx.stroke();
      ctx.fillText(val, padLeft - 5, y + 3);
    }

    // Y-axis label (rotated)
    ctx.save();
    ctx.translate(12, padTop + chartH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = '#9e9891';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.textAlign = 'center';
    ctx.fillText('Words per sentence', 0, 0);
    ctx.restore();

    // Average line
    const avg = raw.reduce((a, b) => a + b, 0) / raw.length;
    const avgY = padTop + chartH * (1 - avg / maxVal);
    ctx.strokeStyle = '#d4a574';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(padLeft, avgY);
    ctx.lineTo(logicalW - padRight, avgY);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#d4a574';
    ctx.textAlign = 'left';
    ctx.font = '9px Source Sans 3, system-ui';
    ctx.fillText(`avg ${avg.toFixed(1)}`, logicalW - padRight + 2, avgY + 3);

    // Fill area under curve
    ctx.beginPath();
    ctx.moveTo(padLeft, padTop + chartH);
    for (let i = 0; i < vals.length; i++) {
      const x = padLeft + i * stepX;
      const y = padTop + chartH * (1 - vals[i] / maxVal);
      ctx.lineTo(x, y);
    }
    ctx.lineTo(padLeft + (vals.length - 1) * stepX, padTop + chartH);
    ctx.closePath();
    ctx.fillStyle = 'rgba(45, 106, 94, 0.08)';
    ctx.fill();

    // Line
    ctx.beginPath();
    ctx.strokeStyle = '#2d6a5e';
    ctx.lineWidth = 1.5;
    ctx.lineJoin = 'round';
    for (let i = 0; i < vals.length; i++) {
      const x = padLeft + i * stepX;
      const y = padTop + chartH * (1 - vals[i] / maxVal);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // X-axis label
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'center';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillText(`${raw.length} sentences`, padLeft + drawW / 2, logicalH - 5);

    // Variation score
    const stddev = Math.sqrt(raw.reduce((sum, v) => sum + (v - avg) ** 2, 0) / raw.length);
    const cv = avg > 0 ? (stddev / avg * 100).toFixed(0) : 0;
    ctx.textAlign = 'right';
    ctx.fillStyle = '#6b6560';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillText(`variation: ${cv}%`, logicalW - padRight, logicalH - 5);
  }

  function handleMouseMove(e, chapterId) {
    const meta = chartMeta[chapterId];
    const overlay = overlays[chapterId];
    if (!meta || !overlay) return;

    const rect = overlay.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const ctx = overlay.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, meta.logicalW, meta.logicalH);

    // Find nearest sentence
    const idx = Math.round((mouseX - meta.padLeft) / meta.stepX);
    if (idx < 0 || idx >= meta.raw.length) {
      overlay.style.cursor = 'default';
      return;
    }

    const x = meta.padLeft + idx * meta.stepX;
    const y = meta.padTop + meta.chartH * (1 - meta.vals[idx] / meta.maxVal);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);

    if (dist > 30) {
      overlay.style.cursor = 'default';
      return;
    }

    overlay.style.cursor = 'pointer';

    // Dot
    ctx.beginPath();
    ctx.arc(x, y, 4, 0, Math.PI * 2);
    ctx.fillStyle = '#2d6a5e';
    ctx.fill();
    ctx.strokeStyle = 'white';
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // Tooltip
    const words = meta.raw[idx];
    const label = `${words} word${words !== 1 ? 's' : ''} — click to go`;
    ctx.font = '11px Source Sans 3, system-ui';
    const tw = ctx.measureText(label).width;
    const tipW = tw + 12;
    const tipH = 22;
    let tipX = x - tipW / 2;
    let tipY = y - tipH - 8;
    if (tipX < 2) tipX = 2;
    if (tipX + tipW > meta.logicalW - 2) tipX = meta.logicalW - tipW - 2;
    if (tipY < 2) tipY = y + 12;

    ctx.fillStyle = 'rgba(45, 42, 38, 0.9)';
    ctx.beginPath();
    ctx.roundRect(tipX, tipY, tipW, tipH, 4);
    ctx.fill();

    ctx.fillStyle = 'white';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, tipX + tipW / 2, tipY + tipH / 2);
    ctx.textBaseline = 'alphabetic';
  }

  function handleMouseLeave(chapterId) {
    const overlay = overlays[chapterId];
    if (!overlay) return;
    const meta = chartMeta[chapterId];
    if (!meta) return;
    const ctx = overlay.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, meta.logicalW, meta.logicalH);
    overlay.style.cursor = 'default';
  }

  async function handleClick(e, chapterId) {
    const meta = chartMeta[chapterId];
    const overlay = overlays[chapterId];
    if (!meta || !overlay) return;

    const rect = overlay.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const idx = Math.round((mouseX - meta.padLeft) / meta.stepX);
    if (idx < 0 || idx >= meta.raw.length) return;

    const x = meta.padLeft + idx * meta.stepX;
    const y = meta.padTop + meta.chartH * (1 - meta.vals[idx] / meta.maxVal);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);
    if (dist > 30) return;

    const charPos = meta.chapter.sentence_starts[idx];
    try {
      const nextStart = idx + 1 < meta.chapter.sentence_starts.length
        ? meta.chapter.sentence_starts[idx + 1]
        : charPos + 1;

      await emitTo('main', 'navigate-to-position', {
        chapterId: meta.chapter.chapter_id,
        charStart: charPos,
        charEnd: nextStart,
      });
      await getCurrentWindow().minimize();
    } catch (err) {
      console.warn('Failed to emit navigation event:', err);
    }
  }
</script>

<div class="pacing-page">
  <header class="pacing-header">
    <h1>Pacing Analysis</h1>
    <p class="pacing-desc">Sentence length waveforms reveal the rhythm of your prose. Peaks are long, flowing sentences. Valleys are short, punchy ones. Variety creates momentum.</p>
    <div class="pacing-controls">
      <label class="smooth-label">
        Smoothing
        <input type="range" class="smooth-slider" bind:value={smoothing} min="1" max="20" step="1" />
        <span class="smooth-val">{smoothing === 1 ? 'Off' : smoothing + ' pt'}</span>
      </label>
      <span class="smooth-hint">Rolling average over this many sentences — higher values reveal broader pacing trends</span>
    </div>
  </header>

  {#if loading}
    <div class="pacing-loading">Analysing pacing...</div>
  {:else if data && data.length > 0}
    <div class="pacing-content">
      {#each data as chapter (chapter.chapter_id)}
        {@const avg = chapter.sentence_lengths.length > 0 ? (chapter.sentence_lengths.reduce((a, b) => a + b, 0) / chapter.sentence_lengths.length) : 0}
        {@const maxS = chapter.sentence_lengths.length > 0 ? Math.max(...chapter.sentence_lengths) : 0}
        {@const minS = chapter.sentence_lengths.length > 0 ? Math.min(...chapter.sentence_lengths) : 0}
        <div class="chapter-section">
          <div class="chapter-heading">
            <h2 class="chapter-title">{chapter.chapter_title}</h2>
            <div class="chapter-stats">
              <span class="stat"><strong>{chapter.sentence_lengths.length}</strong> sentences</span>
              <span class="stat">avg <strong>{avg.toFixed(1)}</strong> words</span>
              <span class="stat">range <strong>{minS}</strong>&ndash;<strong>{maxS}</strong></span>
            </div>
          </div>
          {#if chapter.sentence_lengths.length > 0}
            <div class="chart-wrap">
              <div class="chart-stack">
                <canvas bind:this={canvases[chapter.chapter_id]}></canvas>
                <canvas
                  class="chart-overlay"
                  bind:this={overlays[chapter.chapter_id]}
                  onmousemove={(e) => handleMouseMove(e, chapter.chapter_id)}
                  onmouseleave={() => handleMouseLeave(chapter.chapter_id)}
                  onclick={(e) => handleClick(e, chapter.chapter_id)}
                ></canvas>
              </div>
            </div>
          {:else}
            <div class="chapter-empty">No sentences found</div>
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <div class="pacing-loading">No chapters found.</div>
  {/if}
</div>

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }

  .pacing-page {
    display: flex; flex-direction: column; min-height: 100vh;
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #fffef9; color: #2d2a26;
  }

  .pacing-header {
    padding: 1.5rem 2rem 1rem;
    border-bottom: 1px solid #e5e1da;
    background: #faf8f5;
  }
  .pacing-header h1 {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.4rem; font-weight: 400; margin: 0 0 0.3rem;
  }
  .pacing-desc {
    font-size: 0.8rem; color: #9e9891; margin: 0 0 0.8rem; line-height: 1.5;
    max-width: 700px;
  }
  .pacing-controls {
    display: flex; align-items: center; gap: 1rem;
  }
  .smooth-label {
    display: flex; align-items: center; gap: 0.5rem;
    font-size: 0.75rem; font-weight: 600; color: #6b6560;
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .smooth-slider { accent-color: #2d6a5e; width: 100px; }
  .smooth-val { font-size: 0.75rem; color: #9e9891; min-width: 35px; }
  .smooth-hint { font-size: 0.7rem; color: #9e9891; font-style: italic; font-weight: 400; text-transform: none; letter-spacing: normal; }

  .pacing-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic;
  }

  .pacing-content { padding: 1.5rem 2rem; }

  .chapter-section {
    margin-bottom: 2rem;
  }
  .chapter-heading {
    display: flex; align-items: baseline; gap: 1rem; margin-bottom: 0.5rem;
    flex-wrap: wrap;
  }
  .chapter-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 400; margin: 0; color: #2d2a26;
  }
  .chapter-stats {
    display: flex; gap: 0.8rem;
    font-size: 0.75rem; color: #9e9891;
  }
  .chapter-stats strong { color: #6b6560; }

  .chart-wrap {
    overflow-x: auto; border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem;
  }
  .chart-stack {
    position: relative; width: 100%;
  }
  .chart-stack canvas { display: block; }
  .chart-overlay {
    position: absolute; top: 0; left: 0;
  }

  .chapter-empty {
    font-size: 0.8rem; color: #9e9891; font-style: italic;
    padding: 1rem; text-align: center;
  }
</style>
