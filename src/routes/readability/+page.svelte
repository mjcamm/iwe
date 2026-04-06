<script>
  import { onMount } from 'svelte';
  import { readabilityAnalysis, getLibraryBook, getProjectSetting } from '$lib/db.js';
  import { MIN_BAR_PX } from '$lib/chartConstants.js';
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let data = $state(null);
  let loading = $state(true);
  let canvases = $state({});
  let overlays = $state({});
  let smoothing = $state(5);
  let overviewCanvas = $state(null);

  // Comparison book state
  let compareBook = $state(null);          // { title, chapters: [{ grade_level, total_words }], manuscript_grade }
  let standardiseLength = $state(true);

  let chartMeta = {};

  onMount(async () => {
    try {
      data = await readabilityAnalysis();
    } catch (e) {
      console.warn('Readability analysis failed:', e);
    }
    // Load comparison book from the project's saved setting
    let compareId = null;
    try {
      const stored = await getProjectSetting('comparative_book_id');
      if (stored != null && stored !== '') compareId = parseInt(stored, 10);
    } catch (e) {
      console.warn('Failed to read comparative_book_id setting:', e);
    }
    if (compareId && !Number.isNaN(compareId)) {
      try {
        const book = await getLibraryBook(compareId);
        if (book) {
          const analyses = JSON.parse(book.analysesJson || '{}');
          const r = analyses.readability;
          if (r && Array.isArray(r.chapters)) {
            compareBook = {
              title: book.title,
              author: book.author,
              manuscript_grade: r.manuscript_grade,
              chapters: r.chapters.map(c => ({
                grade_level: c.grade_level,
                total_words: c.total_words || 0
              }))
            };
          }
        }
      } catch (e) {
        console.warn('Failed to load comparison book:', e);
      }
    }
    loading = false;
  });

  /**
   * Standardise: bucket the comparison book's chapters into N word-weighted slices
   * matching the user's chapter boundaries (by % of total words). Returns one
   * grade per user chapter — a word-weighted average of the comparison book's
   * grades over the equivalent slice of its prose.
   */
  function standardisedComparisonGrades(userChapters, compChapters) {
    if (!compChapters || compChapters.length === 0) return [];
    const userWords = userChapters.map(c => c.total_words || 0);
    const compWords = compChapters.map(c => c.total_words || 0);
    const userTotal = userWords.reduce((a, b) => a + b, 0) || 1;
    const compTotal = compWords.reduce((a, b) => a + b, 0) || 1;

    const compRanges = [];
    let cum = 0;
    for (let i = 0; i < compChapters.length; i++) {
      const start = cum / compTotal;
      cum += compWords[i];
      const end = cum / compTotal;
      compRanges.push([start, end]);
    }

    console.groupCollapsed(
      `[readability] standardised comparison: ${userChapters.length} user chapters vs ${compChapters.length} comp chapters`
    );
    console.log(`User total words: ${userTotal.toLocaleString()}, Comp total words: ${compTotal.toLocaleString()}`);

    const out = [];
    let userCum = 0;
    for (let u = 0; u < userChapters.length; u++) {
      const sliceStart = userCum / userTotal;
      userCum += userWords[u];
      const sliceEnd = userCum / userTotal;

      let weightedSum = 0;
      let weightTotal = 0;
      const contributors = [];
      for (let c = 0; c < compChapters.length; c++) {
        const [cs, ce] = compRanges[c];
        const overlap = Math.max(0, Math.min(sliceEnd, ce) - Math.max(sliceStart, cs));
        if (overlap > 0) {
          const w = overlap * compTotal;
          weightedSum += compChapters[c].grade_level * w;
          weightTotal += w;
          contributors.push({
            compChapterIdx: c,
            grade: compChapters[c].grade_level,
            weightWords: Math.round(w)
          });
        }
      }
      const avg = weightTotal > 0 ? weightedSum / weightTotal : 0;

      console.groupCollapsed(
        `User #${u + 1} "${userChapters[u].chapter_title}" — ` +
        `slice ${(sliceStart * 100).toFixed(1)}%–${(sliceEnd * 100).toFixed(1)}% — ` +
        `weighted avg ${avg.toFixed(2)}`
      );
      console.table(contributors);
      console.groupEnd();

      out.push(avg);
    }
    console.log('Final standardised series:', out.map(v => +v.toFixed(2)));
    console.groupEnd();
    return out;
  }

  $effect(() => {
    // Re-draw when data, comparison, or standardise toggle change
    void compareBook;
    void standardiseLength;
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

    const chapters = data.chapters;

    // Comparison only shown when checkbox is ticked.
    const showComparison = !!compareBook && standardiseLength;
    const compChapters = showComparison ? compareBook.chapters : [];
    const slotCount = chapters.length;

    const padTop = 25;
    const padBottom = 140;
    const padLeft = 40;
    const padRight = 60; // extra room for legend / overflow labels
    const chartH = 200;
    const logicalH = padTop + chartH + padBottom;

    const minBarSlot = MIN_BAR_PX * 2.2; // matches MIN_BAR_PX with breathing room for the slot
    const naturalW = padLeft + padRight + slotCount * minBarSlot;
    const logicalW = Math.max(containerW, naturalW, 400);

    const drawW = logicalW - padLeft - padRight;
    const barGap = 4;
    const barW = Math.max(6, (drawW - barGap * (slotCount - 1)) / slotCount);

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);

    // Background
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Compute comparison series (standardised against user chapter slices)
    let compSeries = null;
    if (showComparison) {
      compSeries = standardisedComparisonGrades(chapters, compChapters);
    }

    // Find max grade for scale (include comparison grades)
    const compMax = compSeries ? Math.max(...compSeries.filter(v => v != null)) : 0;
    const maxGrade = Math.max(12, Math.ceil(compMax) + 2,
      ...chapters.map(c => Math.ceil(c.grade_level) + 2));

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
    ctx.strokeStyle = '#7ba89e';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([5, 5]);
    ctx.beginPath();
    ctx.moveTo(padLeft, avgY);
    ctx.lineTo(logicalW - padRight, avgY);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#7ba89e';
    ctx.textAlign = 'left';
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillText(`avg ${data.manuscript_grade.toFixed(1)}`, logicalW - padRight + 2, avgY - 4);

    // Bars
    for (let i = 0; i < chapters.length; i++) {
      const ch = chapters[i];
      const x = padLeft + i * (barW + barGap);
      // Suppress duplicate code path — slot index === chapter index for user bars
      const colH = Math.max(2, (ch.grade_level / maxGrade) * chartH);
      const y = padTop + chartH - colH;

      ctx.fillStyle = '#2d6a5e';
      ctx.beginPath();
      ctx.roundRect(x, y, barW, colH, [3, 3, 0, 0]);
      ctx.fill();

      // Grade value above bar
      ctx.fillStyle = '#2d2a26';
      ctx.font = 'bold 13px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      ctx.fillText(ch.grade_level.toFixed(1), x + barW / 2, y - 6);

      // Chapter label below (rotated steeply for vertical readability)
      ctx.save();
      ctx.translate(x + barW / 2, padTop + chartH + 8);
      ctx.rotate(-Math.PI / 2.4); // ~75° upward
      ctx.fillStyle = '#6b6560';
      ctx.font = '13px Source Sans 3, system-ui';
      ctx.textAlign = 'right';
      const label = ch.chapter_title.length > 36 ? ch.chapter_title.slice(0, 36) + '…' : ch.chapter_title;
      ctx.fillText(label, 0, 4);
      ctx.restore();
    }

    // Comparison overlay — orange line + dots
    if (compSeries && compSeries.length > 0) {
      const orange = '#a85a04';
      // Slot positions for the comparison series
      const slotX = (i) => padLeft + i * (barW + barGap) + barW / 2;

      ctx.strokeStyle = orange;
      ctx.lineWidth = 2;
      ctx.beginPath();
      let started = false;
      for (let i = 0; i < compSeries.length; i++) {
        const v = compSeries[i];
        if (v == null || isNaN(v)) continue;
        const x = slotX(i);
        const y = padTop + chartH * (1 - v / maxGrade);
        if (!started) { ctx.moveTo(x, y); started = true; }
        else ctx.lineTo(x, y);
      }
      ctx.stroke();

      // Dots + value labels — avoid colliding with the green bar's value label
      for (let i = 0; i < compSeries.length; i++) {
        const v = compSeries[i];
        if (v == null || isNaN(v)) continue;
        const userGrade = chapters[i].grade_level;
        const x = slotX(i);
        const y = padTop + chartH * (1 - v / maxGrade);
        ctx.fillStyle = orange;
        ctx.beginPath();
        ctx.arc(x, y, 3.5, 0, Math.PI * 2);
        ctx.fill();

        // Green bar value sits at (greenBarTopY - 5). Push orange label out of the way.
        const greenBarTopY = padTop + chartH * (1 - userGrade / maxGrade);
        const defaultLabelY = y + 14;
        let labelY;
        const greenLabelY = greenBarTopY - 5;

        if (Math.abs(defaultLabelY - greenLabelY) < 12) {
          // Collision — put orange label above the dot instead
          labelY = y - 8;
        } else {
          labelY = defaultLabelY;
        }

        // White rounded box behind label so it's readable on the green bar
        ctx.font = 'bold 12px Source Sans 3, system-ui';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        const labelText = v.toFixed(1);
        const tw = ctx.measureText(labelText).width;
        const boxW = tw + 8;
        const boxH = 16;
        const boxX = x - boxW / 2;
        const boxY = labelY - boxH / 2 + 1;
        ctx.fillStyle = 'rgba(255, 255, 255, 0.95)';
        ctx.strokeStyle = orange;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.roundRect(boxX, boxY, boxW, boxH, 3);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = orange;
        ctx.fillText(labelText, x, labelY + 1);
        ctx.textBaseline = 'alphabetic';
      }

      // Legend
      ctx.font = '11px Source Sans 3, system-ui';
      ctx.textAlign = 'left';
      ctx.fillStyle = orange;
      ctx.fillRect(padLeft, 6, 14, 2);
      ctx.fillText(`${compareBook.title} (standardised)`, padLeft + 20, 11);
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
    ctx.strokeStyle = '#7ba89e';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(padLeft, avgY);
    ctx.lineTo(logicalW - padRight, avgY);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#7ba89e';
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
        <div class="rd-grade-col">
          <div class="rd-grade-big">
            Grade {data.manuscript_grade.toFixed(1)}
          </div>
          {#if compareBook && compareBook.manuscript_grade != null}
            <div class="rd-grade-compare">
              vs <strong>{compareBook.title}</strong> — Grade {compareBook.manuscript_grade.toFixed(1)}
            </div>
          {/if}
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
        {#if compareBook}
          <label class="cmp-label">
            <input type="checkbox" bind:checked={standardiseLength} />
            Standardise comparison book length vs <strong>{compareBook.title}</strong>
          </label>
        {/if}
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
  .rd-grade-col { display: flex; flex-direction: column; }
  .rd-grade-big {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 2.4rem; font-weight: 700; color: #2d2a26;
    line-height: 1.1;
  }
  .rd-grade-compare {
    font-size: 0.78rem; color: #9e9891; margin-top: 0.15rem;
  }
  .rd-grade-compare strong { color: #a85a04; font-weight: 600; }
  .rd-grade-label {
    font-size: 0.85rem; color: #6b6560; font-style: italic;
  }
  .rd-grade-stats {
    font-size: 0.75rem; color: #9e9891;
  }

  .rd-controls {
    display: flex; align-items: center; gap: 0.6rem;
    flex-wrap: wrap;
  }
  .smooth-label {
    display: inline-flex; align-items: center; gap: 0.5rem;
    font-size: 0.92rem; font-weight: 600; color: #6b6560;
  }
  .smooth-slider { accent-color: #2d6a5e; width: 120px; }
  .smooth-val { font-size: 0.9rem; color: #9e9891; min-width: 42px; }

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
    overflow-x: auto;
    width: 100%;
  }
  .overview-chart-wrap canvas { display: block; }
  .cmp-label {
    display: inline-flex; align-items: center; gap: 0.45rem;
    font-size: 0.92rem; color: #6b6560; cursor: pointer;
    white-space: nowrap;
  }
  .cmp-label input {
    accent-color: #a85a04; cursor: pointer;
    width: 16px; height: 16px;
  }
  .cmp-name {
    font-size: 0.92rem; color: #9e9891; white-space: nowrap;
  }
  .cmp-name strong { color: #a85a04; font-weight: 600; }

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
