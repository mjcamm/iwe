<script>
  import { onMount } from 'svelte';
  import { pacingAnalysis, getProjectSetting, getLibraryBook } from '$lib/db.js';
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let data = $state(null);          // raw per-chapter array from Rust
  let loading = $state(true);
  let smoothing = $state(15);       // higher default — whole-book view needs heavier smoothing
  let zoom = $state(1);             // 1 = natural (4px/sentence), lower = more condensed overview
  let canvas = $state(null);        // main waveform canvas
  let overlay = $state(null);       // hover/click canvas

  // Comparison
  let compareBook = $state(null);   // { title, stream: [lengths...] }
  let showComparison = $state(true);

  // Flat sentence stream built from all chapters in order
  // [{ value, chapterId, chapterTitle, sentenceText, charStart }]
  let stream = $derived.by(() => {
    if (!data) return [];
    const out = [];
    for (const ch of data) {
      const lengths = ch.sentence_lengths || [];
      const starts = ch.sentence_starts || [];
      const texts = ch.sentence_texts || [];
      for (let i = 0; i < lengths.length; i++) {
        out.push({
          value: lengths[i],
          chapterId: ch.chapter_id,
          chapterTitle: ch.chapter_title,
          sentenceText: texts[i] || '',
          charStart: starts[i] || 0,
        });
      }
    }
    return out;
  });

  // Chapter boundaries: array of { startIndex, title }
  let chapterBoundaries = $derived.by(() => {
    if (!data) return [];
    const out = [];
    let idx = 0;
    for (const ch of data) {
      out.push({ startIndex: idx, title: ch.chapter_title });
      idx += (ch.sentence_lengths || []).length;
    }
    return out;
  });

  let meta = {};

  onMount(async () => {
    try {
      data = await pacingAnalysis();
    } catch (e) {
      console.warn('Pacing analysis failed:', e);
    }
    // Load comparison book
    let compareId = null;
    try {
      const stored = await getProjectSetting('comparative_book_id');
      if (stored != null && stored !== '') compareId = parseInt(stored, 10);
    } catch (e) {
      console.warn('Failed to read comparative_book_id:', e);
    }
    if (compareId && !Number.isNaN(compareId)) {
      try {
        const book = await getLibraryBook(compareId);
        if (book) {
          const analyses = JSON.parse(book.analysesJson || '{}');
          const pacing = analyses.pacing;
          if (Array.isArray(pacing)) {
            const flat = [];
            for (const ch of pacing) {
              const lengths = ch.sentence_lengths || [];
              for (const v of lengths) flat.push(v);
            }
            if (flat.length > 0) {
              compareBook = { title: book.title, author: book.author, stream: flat };
            }
          }
        }
      } catch (e) {
        console.warn('Failed to load comparison book:', e);
      }
    }
    loading = false;
  });

  /**
   * Squish a comparison stream of length M down to length N by bucketing.
   * Bucket i covers comp[i*M/N .. (i+1)*M/N] and contributes the mean of
   * those values. The result is perfectly aligned to the user's stream
   * index-for-index.
   */
  function squishStream(compStream, targetLength) {
    if (!compStream || compStream.length === 0 || targetLength <= 0) return [];
    const out = new Array(targetLength);
    const m = compStream.length;
    for (let i = 0; i < targetLength; i++) {
      const start = Math.floor((i * m) / targetLength);
      const end = Math.max(start + 1, Math.floor(((i + 1) * m) / targetLength));
      let sum = 0, count = 0;
      for (let j = start; j < end && j < m; j++) {
        sum += compStream[j];
        count++;
      }
      out[i] = count > 0 ? sum / count : 0;
    }
    return out;
  }

  $effect(() => {
    void smoothing;
    void stream;
    void zoom;
    void compareBook;
    void showComparison;
    if (canvas && stream.length > 0) drawWaveform();
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

  function drawWaveform() {
    if (!canvas || stream.length === 0) return;

    const raw = stream.map(s => s.value);
    const vals = smooth(raw, smoothing);

    // Comparison series: squish comp stream to same length, then smooth.
    const overlayActive = !!(compareBook && showComparison && compareBook.stream?.length > 0);
    const compVals = overlayActive
      ? smooth(squishStream(compareBook.stream, raw.length), smoothing)
      : null;

    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 800;
    const BASE_PX_PER_SENTENCE = 4;
    const pxPerSentence = BASE_PX_PER_SENTENCE * zoom;
    const naturalW = 60 + raw.length * pxPerSentence;
    // When zoomed all the way out, the chart shrinks until it fits the viewport.
    const logicalW = Math.max(containerW, naturalW);

    const chartH = 380; // taller per request
    const padTop = 30;
    const padBottom = 60; // room for chapter labels
    const padLeft = 50;
    const padRight = 20;
    const logicalH = chartH + padTop + padBottom;
    const drawW = logicalW - padLeft - padRight;
    const stepX = drawW / Math.max(raw.length - 1, 1);

    // Shared y-scale includes the comparison values
    const allVals = compVals ? [...vals, ...compVals] : vals;
    const maxVal = Math.max(1, ...allVals.map(v => Math.ceil(v)));

    meta = { padLeft, padTop, chartH, drawW, stepX, maxVal, raw, vals, logicalW, logicalH, padRight };

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);

    if (overlay) {
      overlay.width = logicalW * dpr;
      overlay.height = logicalH * dpr;
      overlay.style.width = logicalW + 'px';
      overlay.style.height = logicalH + 'px';
    }

    // Background
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Y-axis grid
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 0.5;
    ctx.font = '11px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    const gridSteps = 5;
    for (let i = 0; i <= gridSteps; i++) {
      const y = padTop + (chartH * i / gridSteps);
      const val = Math.round(maxVal * (1 - i / gridSteps));
      ctx.beginPath();
      ctx.moveTo(padLeft, y);
      ctx.lineTo(logicalW - padRight, y);
      ctx.stroke();
      ctx.fillText(val, padLeft - 6, y + 4);
    }

    // Y-axis label
    ctx.save();
    ctx.translate(15, padTop + chartH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = '#9e9891';
    ctx.font = '11px Source Sans 3, system-ui';
    ctx.textAlign = 'center';
    ctx.fillText('Words per sentence', 0, 0);
    ctx.restore();

    // Chapter boundary lines + labels
    ctx.font = '11px Source Sans 3, system-ui';
    for (let i = 0; i < chapterBoundaries.length; i++) {
      const b = chapterBoundaries[i];
      const x = padLeft + b.startIndex * stepX;
      // Skip the very first boundary (always at idx 0) for the line, but label it
      if (i > 0) {
        ctx.strokeStyle = 'rgba(45, 42, 38, 0.18)';
        ctx.lineWidth = 1;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.moveTo(x, padTop);
        ctx.lineTo(x, padTop + chartH);
        ctx.stroke();
        ctx.setLineDash([]);
      }
      // Label rotated 75° at the bottom
      ctx.save();
      ctx.translate(x + 4, padTop + chartH + 6);
      ctx.rotate(-Math.PI / 2.4);
      ctx.fillStyle = '#6b6560';
      ctx.textAlign = 'right';
      const label = b.title.length > 28 ? b.title.slice(0, 28) + '…' : b.title;
      ctx.fillText(label, 0, 4);
      ctx.restore();
    }

    // Manuscript average line
    const avg = raw.reduce((a, b) => a + b, 0) / raw.length;
    const avgY = padTop + chartH * (1 - avg / maxVal);
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
    ctx.font = '11px Source Sans 3, system-ui';
    ctx.fillText(`avg ${avg.toFixed(1)}`, logicalW - padRight - 56, avgY - 5);

    // Filled area under curve
    ctx.beginPath();
    ctx.moveTo(padLeft, padTop + chartH);
    for (let i = 0; i < vals.length; i++) {
      const x = padLeft + i * stepX;
      const y = padTop + chartH * (1 - vals[i] / maxVal);
      ctx.lineTo(x, y);
    }
    ctx.lineTo(padLeft + (vals.length - 1) * stepX, padTop + chartH);
    ctx.closePath();
    ctx.fillStyle = 'rgba(45, 106, 94, 0.10)';
    ctx.fill();

    // Waveform line
    ctx.beginPath();
    ctx.strokeStyle = '#2d6a5e';
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    for (let i = 0; i < vals.length; i++) {
      const x = padLeft + i * stepX;
      const y = padTop + chartH * (1 - vals[i] / maxVal);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Comparison overlay — orange line (no fill, so it sits cleanly over the green)
    if (compVals) {
      const orange = '#a85a04';
      ctx.beginPath();
      ctx.strokeStyle = orange;
      ctx.lineWidth = 1.8;
      ctx.lineJoin = 'round';
      for (let i = 0; i < compVals.length; i++) {
        const x = padLeft + i * stepX;
        const y = padTop + chartH * (1 - compVals[i] / maxVal);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();

      // Legend (top-right)
      ctx.font = '12px Source Sans 3, system-ui';
      ctx.textAlign = 'right';
      const legendText = `${compareBook.title} (squished)`;
      const tw = ctx.measureText(legendText).width;
      const lx = logicalW - padRight - tw - 22;
      ctx.fillStyle = orange;
      ctx.fillRect(lx, 12, 14, 2);
      ctx.textAlign = 'left';
      ctx.fillText(legendText, lx + 20, 16);
    }
  }

  function handleMouseMove(e) {
    if (!overlay || !meta.raw) return;
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
    const y = meta.padTop + meta.chartH * (1 - meta.vals[idx] / meta.maxVal);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);
    if (dist > 40) {
      overlay.style.cursor = 'default';
      return;
    }
    overlay.style.cursor = 'pointer';

    // Dot
    ctx.beginPath();
    ctx.arc(x, y, 5, 0, Math.PI * 2);
    ctx.fillStyle = '#2d6a5e';
    ctx.fill();
    ctx.strokeStyle = 'white';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Tooltip
    const s = stream[idx];
    const label = `${s.value} word${s.value !== 1 ? 's' : ''} — ${s.chapterTitle}`;
    ctx.font = '12px Source Sans 3, system-ui';
    const tw = ctx.measureText(label).width;
    const tipW = tw + 16;
    const tipH = 26;
    let tipX = x - tipW / 2;
    let tipY = y - tipH - 10;
    if (tipX < 4) tipX = 4;
    if (tipX + tipW > meta.logicalW - 4) tipX = meta.logicalW - tipW - 4;
    if (tipY < 4) tipY = y + 14;

    ctx.fillStyle = 'rgba(45, 42, 38, 0.92)';
    ctx.beginPath();
    ctx.roundRect(tipX, tipY, tipW, tipH, 5);
    ctx.fill();
    ctx.fillStyle = 'white';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, tipX + tipW / 2, tipY + tipH / 2);
    ctx.textBaseline = 'alphabetic';
  }

  function handleMouseLeave() {
    if (!overlay) return;
    const ctx = overlay.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, meta.logicalW, meta.logicalH);
    overlay.style.cursor = 'default';
  }

  async function handleClick(e) {
    if (!overlay || !meta.raw) return;
    const rect = overlay.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const idx = Math.round((mouseX - meta.padLeft) / meta.stepX);
    if (idx < 0 || idx >= meta.raw.length) return;

    const x = meta.padLeft + idx * meta.stepX;
    const y = meta.padTop + meta.chartH * (1 - meta.vals[idx] / meta.maxVal);
    const dist = Math.sqrt((mouseX - x) ** 2 + (mouseY - y) ** 2);
    if (dist > 40) return;

    const s = stream[idx];
    try {
      await emitTo('main', 'navigate-to-position', {
        chapterId: s.chapterId,
        searchText: s.sentenceText,
        charPosition: s.charStart,
      });
      await getCurrentWindow().minimize();
    } catch (err) {
      console.warn('Failed to emit navigation event:', err);
    }
  }

  // Stats for header
  let totalSentences = $derived(stream.length);
  let avgLen = $derived(stream.length > 0 ? (stream.reduce((a, s) => a + s.value, 0) / stream.length) : 0);
</script>

<div class="pacing-page">
  <header class="pacing-header">
    <h1>Pacing Analysis</h1>
    <p class="pacing-desc">The whole-book waveform of sentence length, end to end. <strong>Higher on the chart</strong> means longer sentences — slower, more contemplative pacing. <strong>Lower on the chart</strong> means shorter sentences — faster, punchier pacing. Dashed vertical lines mark chapter boundaries. Increase smoothing to see the macro shape of the book.</p>
    <div class="pacing-controls">
      <label class="smooth-label">
        Smoothing
        <input type="range" class="smooth-slider" bind:value={smoothing} min="1" max="150" step="1" />
        <span class="smooth-val">{smoothing === 1 ? 'Off' : smoothing + ' pt'}</span>
      </label>
      <label class="smooth-label">
        Zoom
        <input type="range" class="smooth-slider" bind:value={zoom} min="0.05" max="1" step="0.01" />
        <span class="smooth-val">{Math.round(zoom * 100)}%</span>
      </label>
      {#if data}
        <span class="hdr-stat"><strong>{totalSentences.toLocaleString()}</strong> sentences</span>
        <span class="hdr-stat">avg <strong>{avgLen.toFixed(1)}</strong> words</span>
        <span class="hdr-stat"><strong>{chapterBoundaries.length}</strong> chapters</span>
      {/if}
      {#if compareBook}
        <label class="cmp-toggle">
          <input type="checkbox" bind:checked={showComparison} />
          Compare vs <strong>{compareBook.title}</strong>
        </label>
      {/if}
    </div>
    {#if compareBook && showComparison}
      <p class="cmp-explainer">
        <strong>{compareBook.title}</strong>'s pacing waveform has been compressed (or stretched) to exactly
        match the length of your manuscript — each point of their waveform is averaged into the equivalent
        position of yours, so the two books can be compared shape-for-shape at every % through the book.
      </p>
    {/if}
  </header>

  {#if loading}
    <div class="pacing-loading">Analysing pacing...</div>
  {:else if data && stream.length > 0}
    <div class="pacing-content">
      <div class="chart-wrap">
        <div class="chart-stack">
          <canvas bind:this={canvas}></canvas>
          <canvas
            class="chart-overlay"
            bind:this={overlay}
            onmousemove={handleMouseMove}
            onmouseleave={handleMouseLeave}
            onclick={handleClick}
          ></canvas>
        </div>
      </div>
    </div>
  {:else}
    <div class="pacing-loading">No sentences found.</div>
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
    font-size: 1rem; color: #6b6560; margin: 0 0 1rem; line-height: 1.55;
    max-width: 920px;
  }
  .pacing-desc strong { color: #2d2a26; font-weight: 600; }
  .cmp-explainer {
    margin: 0.9rem 0 0; font-size: 0.95rem; line-height: 1.55;
    color: #6b6560; max-width: 920px;
    padding: 0.7rem 0.9rem; background: rgba(168, 90, 4, 0.08);
    border-left: 3px solid #a85a04; border-radius: 4px;
  }
  .cmp-explainer strong { color: #a85a04; font-weight: 600; }
  .pacing-controls {
    display: flex; align-items: center; gap: 1rem; flex-wrap: wrap;
  }
  .smooth-label {
    display: inline-flex; align-items: center; gap: 0.5rem;
    font-size: 0.92rem; font-weight: 600; color: #6b6560;
  }
  .smooth-slider { accent-color: #2d6a5e; width: 160px; }
  .smooth-val { font-size: 0.9rem; color: #9e9891; min-width: 50px; }
  .hdr-stat {
    font-size: 0.88rem; color: #9e9891;
    padding-left: 0.8rem; border-left: 1px solid #e5e1da;
  }
  .hdr-stat strong { color: #6b6560; font-weight: 600; }
  .cmp-toggle {
    display: inline-flex; align-items: center; gap: 0.45rem;
    font-size: 0.92rem; color: #6b6560; cursor: pointer;
    padding-left: 0.8rem; border-left: 1px solid #e5e1da;
    white-space: nowrap;
  }
  .cmp-toggle input { accent-color: #a85a04; width: 16px; height: 16px; }
  .cmp-toggle strong { color: #a85a04; font-weight: 600; }

  .pacing-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic;
  }

  .pacing-content { padding: 1.5rem 2rem 2rem; }

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
</style>
