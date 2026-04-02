<script>
  import { onMount } from 'svelte';
  import { readabilityAnalysis } from '$lib/db.js';
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let data = $state(null);
  let loading = $state(true);
  let canvases = $state({});
  let overlays = $state({});
  let smoothing = $state(5);
  let overviewCanvas = $state(null);

  let chartMeta = {};

  onMount(async () => {
    try {
      data = await readabilityAnalysis();
    } catch (e) {
      console.warn('Readability analysis failed:', e);
    }
    loading = false;
  });

  $effect(() => {
    if (!data) return;
    if (overviewCanvas) drawOverviewBar(overviewCanvas);
    for (const ch of data.chapters) {
      const canvas = canvases[ch.chapter_id];
      if (canvas) drawChart(canvas, ch);
    }
  });

  function gradeLabel(grade) {
    if (grade <= 0) return 'Very easy';
    if (grade <= 5) return 'Easy';
    if (grade <= 8) return 'Standard';
    if (grade <= 12) return 'Advanced';
    return 'Very complex';
  }

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

  function drawOverviewBar(canvas) {
    if (!data || data.chapters.length === 0) return;

    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 700;
    const logicalW = Math.max(containerW, 400);

    const chapters = data.chapters;
    const padTop = 25;
    const padBottom = 60;
    const padLeft = 40;
    const padRight = 15;
    const chartH = 200;
    const logicalH = padTop + chartH + padBottom;
    const drawW = logicalW - padLeft - padRight;
    const barGap = 6;
    const barW = Math.max(8, (drawW - barGap * (chapters.length - 1)) / chapters.length);

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);

    // Background
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Find max grade for scale
    const maxGrade = Math.max(12, ...chapters.map(c => Math.ceil(c.grade_level) + 2));

    // Y-axis grid lines + labels
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 0.5;
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    for (let g = 0; g <= maxGrade; g += 2) {
      const y = padTop + chartH * (1 - g / maxGrade);
      ctx.beginPath();
      ctx.moveTo(padLeft, y);
      ctx.lineTo(logicalW - padRight, y);
      ctx.stroke();
      ctx.fillText(g, padLeft - 5, y + 3);
    }

    // Y-axis label
    ctx.save();
    ctx.translate(12, padTop + chartH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = '#9e9891';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.textAlign = 'center';
    ctx.fillText('Grade level', 0, 0);
    ctx.restore();

    // Manuscript average line
    const avgY = padTop + chartH * (1 - data.manuscript_grade / maxGrade);
    ctx.strokeStyle = '#d4a574';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([5, 5]);
    ctx.beginPath();
    ctx.moveTo(padLeft, avgY);
    ctx.lineTo(logicalW - padRight, avgY);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#d4a574';
    ctx.textAlign = 'left';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillText(`avg ${data.manuscript_grade.toFixed(1)}`, logicalW - padRight + 2, avgY - 4);

    // Bars
    for (let i = 0; i < chapters.length; i++) {
      const ch = chapters[i];
      const x = padLeft + i * (barW + barGap);
      const colH = Math.max(2, (ch.grade_level / maxGrade) * chartH);
      const y = padTop + chartH - colH;

      ctx.fillStyle = '#2d6a5e';
      ctx.beginPath();
      ctx.roundRect(x, y, barW, colH, [3, 3, 0, 0]);
      ctx.fill();

      // Grade value above bar
      ctx.fillStyle = '#2d2a26';
      ctx.font = 'bold 11px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      ctx.fillText(ch.grade_level.toFixed(1), x + barW / 2, y - 5);

      // Chapter label below (rotated)
      ctx.save();
      ctx.translate(x + barW / 2, padTop + chartH + 8);
      ctx.rotate(Math.PI / 4);
      ctx.fillStyle = '#6b6560';
      ctx.font = '10px Source Sans 3, system-ui';
      ctx.textAlign = 'left';
      const label = ch.chapter_title.length > 18 ? ch.chapter_title.slice(0, 18) + '...' : ch.chapter_title;
      ctx.fillText(label, 0, 0);
      ctx.restore();
    }
  }

  function drawChart(canvas, chapter) {
    const raw = chapter.sentence_grades;
    if (!raw || raw.length === 0) return;
    const vals = smooth(raw, smoothing);

    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 700;
    const logicalW = Math.max(containerW, 400);
    const chartH = 120;
    const padTop = 25;
    const padBottom = 30;
    const padLeft = 40;
    const padRight = 15;
    const logicalH = chartH + padTop + padBottom;
    const drawW = logicalW - padLeft - padRight;
    const stepX = drawW / Math.max(raw.length - 1, 1);

    const minDisplay = 0;
    const maxDisplay = Math.max(20, ...vals.map(v => Math.ceil(v)));
    const range = maxDisplay - minDisplay;

    chartMeta[chapter.chapter_id] = {
      padLeft, padTop, chartH, drawW, stepX, minDisplay, maxDisplay, range,
      raw, vals, chapter, logicalW, logicalH, padRight
    };

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);

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

    // Grade zone bands
    const zones = [
      { from: 0, to: 5, color: 'rgba(45, 106, 94, 0.04)' },
      { from: 5, to: 8, color: 'rgba(90, 138, 60, 0.04)' },
      { from: 8, to: 12, color: 'rgba(212, 165, 116, 0.04)' },
      { from: 12, to: maxDisplay, color: 'rgba(196, 94, 74, 0.04)' },
    ];
    for (const zone of zones) {
      const y1 = padTop + chartH * (1 - (Math.min(zone.to, maxDisplay) - minDisplay) / range);
      const y2 = padTop + chartH * (1 - (Math.max(zone.from, minDisplay) - minDisplay) / range);
      ctx.fillStyle = zone.color;
      ctx.fillRect(padLeft, y1, drawW, y2 - y1);
    }

    // Y-axis grid lines
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 0.5;
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    for (let g = 0; g <= maxDisplay; g += 4) {
      const y = padTop + chartH * (1 - (g - minDisplay) / range);
      ctx.beginPath();
      ctx.moveTo(padLeft, y);
      ctx.lineTo(logicalW - padRight, y);
      ctx.stroke();
      ctx.fillText(g, padLeft - 5, y + 3);
    }

    // Y-axis label
    ctx.save();
    ctx.translate(12, padTop + chartH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = '#9e9891';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.textAlign = 'center';
    ctx.fillText('Grade level', 0, 0);
    ctx.restore();

    // Chapter average line
    const avg = chapter.grade_level;
    const avgY = padTop + chartH * (1 - (avg - minDisplay) / range);
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
    const baseY = padTop + chartH * (1 - (0 - minDisplay) / range);
    ctx.moveTo(padLeft, baseY);
    for (let i = 0; i < vals.length; i++) {
      const x = padLeft + i * stepX;
      const y = padTop + chartH * (1 - (vals[i] - minDisplay) / range);
      ctx.lineTo(x, y);
    }
    ctx.lineTo(padLeft + (vals.length - 1) * stepX, baseY);
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
      const y = padTop + chartH * (1 - (vals[i] - minDisplay) / range);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // X-axis label
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'center';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillText(`${raw.length} sentences`, padLeft + drawW / 2, logicalH - 5);
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

    const idx = Math.round((mouseX - meta.padLeft) / meta.stepX);
    if (idx < 0 || idx >= meta.raw.length) {
      overlay.style.cursor = 'default';
      return;
    }

    const x = meta.padLeft + idx * meta.stepX;
    const y = meta.padTop + meta.chartH * (1 - (meta.vals[idx] - meta.minDisplay) / meta.range);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);

    if (dist > 30) {
      overlay.style.cursor = 'default';
      return;
    }

    overlay.style.cursor = 'pointer';

    // Dot
    const grade = meta.raw[idx];
    ctx.beginPath();
    ctx.arc(x, y, 4, 0, Math.PI * 2);
    ctx.fillStyle = '#2d6a5e';
    ctx.fill();
    ctx.strokeStyle = 'white';
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // Tooltip
    const label = `Grade ${grade.toFixed(1)} — click to go`;
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
    const y = meta.padTop + meta.chartH * (1 - (meta.vals[idx] - meta.minDisplay) / meta.range);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);
    if (dist > 30) return;

    const sentenceText = meta.chapter.sentence_texts?.[idx] || '';
    const hintPos = meta.chapter.sentence_starts[idx];
    try {
      await emitTo('main', 'navigate-to-position', {
        chapterId: meta.chapter.chapter_id,
        searchText: sentenceText,
        charPosition: hintPos,
      });
      await getCurrentWindow().minimize();
    } catch (err) {
      console.warn('Failed to emit navigation event:', err);
    }
  }
</script>

<div class="rd-page">
  <header class="rd-header">
    <h1>Readability Score</h1>
    <p class="rd-desc">Flesch-Kincaid grade level measures how complex your prose is. Consistency matters more than any single number — large jumps between chapters create a jarring reading experience.</p>
    {#if data && !loading}
      <div class="rd-manuscript">
        <div class="rd-grade-big">
          Grade {data.manuscript_grade.toFixed(1)}
        </div>
        <div class="rd-grade-label">{gradeLabel(data.manuscript_grade)}</div>
        <div class="rd-grade-stats">
          {data.manuscript_words.toLocaleString()} words &middot;
          {data.manuscript_sentences.toLocaleString()} sentences &middot;
          {data.manuscript_syllables.toLocaleString()} syllables
        </div>
      </div>
      <div class="rd-controls">
        <label class="smooth-label">
          Smoothing
          <input type="range" class="smooth-slider" bind:value={smoothing} min="1" max="20" step="1" />
          <span class="smooth-val">{smoothing === 1 ? 'Off' : smoothing + ' pt'}</span>
        </label>
      </div>
    {/if}
  </header>

  {#if loading}
    <div class="rd-loading">Analysing readability...</div>
  {:else if data && data.chapters.length > 0}
    <div class="rd-overview">
      <h2 class="rd-section-title">Per Chapter</h2>
      <div class="overview-chart-wrap">
        <canvas bind:this={overviewCanvas}></canvas>
      </div>
      <div class="rd-chapter-list">
        {#each data.chapters as ch (ch.chapter_id)}
          <div class="rd-chapter-row">
            <span class="rd-ch-title">{ch.chapter_title}</span>
            <span class="rd-ch-grade">
              {ch.grade_level.toFixed(1)}
            </span>
            <span class="rd-ch-detail">
              avg {ch.avg_words_per_sentence} words/sent &middot;
              {ch.total_words.toLocaleString()} words &middot;
              {ch.total_sentences.toLocaleString()} sentences &middot;
              {ch.total_syllables.toLocaleString()} syllables
            </span>
          </div>
        {/each}
      </div>
    </div>

    <div class="rd-charts">
      <h2 class="rd-section-title">Sentence-Level Detail</h2>
      <p class="rd-charts-desc">Each chart shows per-sentence grade level. Click a point to navigate to that sentence in the editor.</p>
      {#each data.chapters as chapter (chapter.chapter_id)}
        <div class="chapter-section">
          <div class="chapter-heading">
            <h3 class="chapter-title">{chapter.chapter_title}</h3>
            <span class="chapter-grade">
              Grade {chapter.grade_level.toFixed(1)}
            </span>
          </div>
          {#if chapter.sentence_grades.length > 0}
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
    <div class="rd-loading">No chapters found.</div>
  {/if}
</div>

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }

  .rd-page {
    display: flex; flex-direction: column; min-height: 100vh;
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #fffef9; color: #2d2a26;
  }

  .rd-header {
    padding: 1.5rem 2rem 1rem;
    border-bottom: 1px solid #e5e1da;
    background: #faf8f5;
  }
  .rd-header h1 {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.4rem; font-weight: 400; margin: 0 0 0.3rem;
  }
  .rd-desc {
    font-size: 0.8rem; color: #9e9891; margin: 0 0 1rem; line-height: 1.5;
    max-width: 700px;
  }

  .rd-manuscript {
    display: flex; align-items: baseline; gap: 0.8rem; flex-wrap: wrap;
    margin-bottom: 0.8rem;
  }
  .rd-grade-big {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 2.4rem; font-weight: 700; color: #2d2a26;
  }
  .rd-grade-label {
    font-size: 0.85rem; color: #6b6560; font-style: italic;
  }
  .rd-grade-stats {
    font-size: 0.75rem; color: #9e9891;
  }

  .rd-controls {
    display: flex; align-items: center; gap: 1rem;
  }
  .smooth-label {
    display: flex; align-items: center; gap: 0.5rem;
    font-size: 0.75rem; font-weight: 600; color: #6b6560;
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .smooth-slider { accent-color: #2d6a5e; width: 100px; }
  .smooth-val { font-size: 0.75rem; color: #9e9891; min-width: 35px; }

  .rd-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic;
  }

  /* Overview */
  .rd-overview { padding: 1.5rem 2rem 0; }
  .rd-section-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 400; margin: 0 0 0.8rem; color: #2d2a26;
  }

  .overview-chart-wrap {
    border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem;
    margin-bottom: 1rem;
  }
  .overview-chart-wrap canvas { display: block; }

  .rd-chapter-list {
    display: flex; flex-direction: column; gap: 0;
    border: 1px solid #e5e1da; border-radius: 6px; overflow: hidden;
  }
  .rd-chapter-row {
    display: flex; align-items: center; gap: 0.8rem;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid #f0ede8;
    font-size: 0.95rem;
  }
  .rd-chapter-row:last-child { border-bottom: none; }
  .rd-chapter-row:nth-child(even) { background: #faf8f5; }
  .rd-ch-title {
    flex: 1; font-weight: 500; color: #2d2a26;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .rd-ch-grade {
    font-weight: 700; font-size: 1.05rem; min-width: 36px; text-align: right;
    color: #2d2a26;
  }
  .rd-ch-detail { color: #6b6560; font-size: 0.85rem; min-width: 280px; text-align: right; }

  /* Charts */
  .rd-charts { padding: 1.5rem 2rem 2rem; }
  .rd-charts-desc {
    font-size: 0.78rem; color: #9e9891; margin: -0.4rem 0 1rem; line-height: 1.4;
  }

  .chapter-section { margin-bottom: 1.5rem; }
  .chapter-heading {
    display: flex; align-items: baseline; gap: 0.8rem; margin-bottom: 0.4rem;
  }
  .chapter-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 0.9rem; font-weight: 400; margin: 0; color: #2d2a26;
  }
  .chapter-grade { font-size: 0.85rem; font-weight: 600; color: #2d2a26; }

  .chart-wrap {
    overflow-x: auto; border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem;
  }
  .chart-stack { position: relative; width: 100%; }
  .chart-stack canvas { display: block; }
  .chart-overlay { position: absolute; top: 0; left: 0; }

  .chapter-empty {
    font-size: 0.8rem; color: #9e9891; font-style: italic;
    padding: 1rem; text-align: center;
  }
</style>
