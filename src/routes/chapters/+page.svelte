<script>
  import { onMount } from 'svelte';
  import { chapterAnalysis, getProjectSetting, getLibraryBook } from '$lib/db.js';
  import { MIN_BAR_PX, MAX_BAR_PX } from '$lib/chartConstants.js';

  let data = $state(null);
  let loading = $state(true);
  let wordsPerMin = $state(250);

  // Comparison
  let compareBook = $state(null);   // { title, author, chapters: [...] }
  let showComparison = $state(true);

  let chapCanvas = $state();
  let dialogueCanvas = $state();
  let sentenceLenCanvas = $state();
  let vocabCanvas = $state();

  // Comparison palette: dark / light orange
  const ORANGE_DARK = '#a85a04';
  const ORANGE_LIGHT = '#e8a35c';

  onMount(async () => {
    try {
      data = await chapterAnalysis();
    } catch (e) {
      console.warn('Chapter analysis failed:', e);
    }
    // Load comparison
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
          const ch = analyses.chapter;
          if (Array.isArray(ch)) {
            compareBook = { title: book.title, author: book.author, chapters: ch };
          }
        }
      } catch (e) {
        console.warn('Failed to load comparison book:', e);
      }
    }
    loading = false;
    console.log('[chapters] data loaded', {
      chapterCount: data?.length,
      firstChapter: data?.[0],
      hasCompare: !!compareBook,
      compareChapterCount: compareBook?.chapters?.length,
    });
  });

  $effect(() => { void compareBook; void showComparison; if (data && chapCanvas) drawWordCountChart(); });
  $effect(() => { void compareBook; void showComparison; if (data && dialogueCanvas) drawDialogueChart(); });
  $effect(() => { void compareBook; void showComparison; if (data && sentenceLenCanvas) drawSentenceLenChart(); });
  $effect(() => { void compareBook; void showComparison; if (data && vocabCanvas) drawVocabChart(); });

  /**
   * Draws a chapter bar chart. If a compareValues array is supplied, each
   * chapter renders as a green bar + an adjacent orange comparison bar grouped
   * together, with a larger gap between groups. Comparison data extends past
   * the manuscript if the comparison book has more chapters.
   */
  function drawBarChart(canvas, values, color, compareValues = null) {
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 600;
    console.log('[chapters] drawBarChart', {
      canvasId: canvas?.id || canvas?.className,
      valuesLen: values?.length,
      sampleValues: values?.slice(0, 5),
      anyNonZero: values?.some(v => v != null && v !== 0),
      compareLen: compareValues?.length,
      containerW, dpr,
    });

    const hasComp = !!compareValues && compareValues.length > 0;
    const slotCount = hasComp ? Math.max(values.length, compareValues.length) : values.length;

    const barW = hasComp
      ? Math.max(MIN_BAR_PX, Math.min(MAX_BAR_PX * 0.6, (containerW - 100) / (slotCount * 2.5)))
      : Math.max(MIN_BAR_PX * 1.6, Math.min(MAX_BAR_PX, (containerW - 80) / slotCount));
    const pairGap = 1;
    const groupGap = hasComp ? Math.max(10, barW * 0.7) : 6;
    const groupW = hasComp ? barW * 2 + pairGap : barW;

    const chartH = 220;
    const labelH = 80;
    const logicalW = Math.max(containerW, 70 + slotCount * (groupW + groupGap));
    const logicalH = chartH + labelH + 30;

    const physicalW = logicalW * dpr;
    const physicalH = logicalH * dpr;
    const MAX_CANVAS = 32767;
    if (physicalW > MAX_CANVAS || physicalH > MAX_CANVAS) {
      console.warn('[chapters] BAR CHART canvas exceeds browser max', {
        physicalW, physicalH, logicalW, logicalH, dpr, slotCount, groupW, groupGap,
      });
    }
    canvas.width = physicalW;
    canvas.height = physicalH;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Shared y-scale across both books
    const allVals = hasComp ? [...values, ...compareValues] : values;
    const filtered = allVals.filter(v => v != null);
    const maxVal = Math.max(...filtered, 1);
    console.log('[chapters] drawBarChart computed', {
      slotCount, barW, groupW, groupGap, logicalW, logicalH, maxVal,
      filteredCount: filtered.length,
    });

    for (let i = 0; i < slotCount; i++) {
      const groupX = 60 + i * (groupW + groupGap);
      const ownVal = values[i];
      const compVal = hasComp ? compareValues[i] : null;

      // Own (manuscript) bar
      if (ownVal != null) {
        const h = (ownVal / maxVal) * chartH;
        const y = chartH - h + 15;
        ctx.fillStyle = color;
        ctx.fillRect(groupX, y, barW, h);
        ctx.fillStyle = '#6b6560';
        ctx.font = '10px Source Sans 3, system-ui';
        ctx.textAlign = 'center';
        ctx.fillText(formatVal(ownVal), groupX + barW / 2, y - 5);
      }

      // Comparison bar (orange)
      if (hasComp && compVal != null) {
        const h = (compVal / maxVal) * chartH;
        const y = chartH - h + 15;
        const cx = groupX + barW + pairGap;
        ctx.fillStyle = ORANGE_DARK;
        ctx.fillRect(cx, y, barW, h);
        ctx.fillStyle = ORANGE_DARK;
        ctx.font = '10px Source Sans 3, system-ui';
        ctx.textAlign = 'center';
        ctx.fillText(formatVal(compVal), cx + barW / 2, y - 5);
      }

      // Chapter name label — use the user's chapter title where present,
      // otherwise the comparison's title for any extra slots
      const titleSource = i < data.length
        ? data[i].chapter_title
        : (compareBook?.chapters[i]?.chapter_title || `#${i + 1}`);
      ctx.save();
      ctx.translate(groupX + groupW / 2, chartH + 25);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = 'right';
      ctx.font = '11px Source Sans 3, system-ui';
      ctx.fillStyle = i < data.length ? '#9e9891' : ORANGE_DARK;
      const label = titleSource.length > 22 ? titleSource.slice(0, 22) + '…' : titleSource;
      ctx.fillText(label, 0, 0);
      ctx.restore();
    }

    // Legend if comparison
    if (hasComp) {
      ctx.fillStyle = color; ctx.fillRect(logicalW - 240, 8, 12, 12);
      ctx.fillStyle = '#6b6560'; ctx.font = '11px Source Sans 3, system-ui'; ctx.textAlign = 'left';
      ctx.fillText('Manuscript', logicalW - 224, 18);
      ctx.fillStyle = ORANGE_DARK; ctx.fillRect(logicalW - 140, 8, 12, 12);
      ctx.fillStyle = '#6b6560'; ctx.fillText(compareBook.title, logicalW - 124, 18);
    }
  }

  function formatVal(v) {
    if (typeof v !== 'number') return String(v);
    if (v % 1 !== 0) return v.toFixed(1);
    return v.toLocaleString();
  }

  function compareValues(getter) {
    if (!compareBook || !showComparison) return null;
    return compareBook.chapters.map(c => getter(c));
  }

  function drawWordCountChart() {
    drawBarChart(
      chapCanvas,
      data.map(d => d.total_words),
      '#2d6a5e',
      compareValues(c => c.total_words)
    );
  }

  function drawDialogueChart() {
    const ctx = dialogueCanvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = dialogueCanvas.parentElement?.clientWidth || 600;

    const hasComp = !!(compareBook && showComparison && compareBook.chapters.length > 0);
    const slotCount = hasComp ? Math.max(data.length, compareBook.chapters.length) : data.length;
    const barsPerGroup = hasComp ? 4 : 2;

    const singleBarW = hasComp
      ? Math.max(MIN_BAR_PX, Math.min(MAX_BAR_PX * 0.5, (containerW - 100) / (slotCount * 5)))
      : Math.max(MIN_BAR_PX, Math.min(MAX_BAR_PX * 0.6, (containerW - 100) / (slotCount * 2.5)));
    const pairGap = 1;
    const groupGap = singleBarW * 1.4;
    const groupW = singleBarW * barsPerGroup + pairGap * (barsPerGroup - 1);

    const chartH = 220;
    const labelH = 90;
    const logicalW = Math.max(containerW, 80 + slotCount * (groupW + groupGap));
    const logicalH = chartH + labelH + 30;

    const physicalW = logicalW * dpr;
    const physicalH = logicalH * dpr;
    const MAX_CANVAS = 32767;
    console.log('[chapters] drawDialogueChart', {
      dataLen: data?.length, hasComp, slotCount, singleBarW, groupW, groupGap,
      logicalW, logicalH, physicalW, physicalH, dpr,
      sampleOwn: data?.slice(0, 3).map(d => ({ t: d.total_words, n: d.narrative_words, dlg: d.dialogue_words })),
    });
    if (physicalW > MAX_CANVAS || physicalH > MAX_CANVAS) {
      console.warn('[chapters] DIALOGUE canvas exceeds browser max', { physicalW, physicalH });
    }
    dialogueCanvas.width = physicalW;
    dialogueCanvas.height = physicalH;
    dialogueCanvas.style.width = logicalW + 'px';
    dialogueCanvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Bars represent percentages of total chapter words, NOT raw counts.
    // This way a chapter with 8k words at 89% narrative is comparable to a
    // chapter with 4k words at 89% narrative — both reach the same height.
    const maxVal = 100;
    const narPctOf = (c) => c && c.total_words > 0 ? (c.narrative_words / c.total_words) * 100 : 0;
    const dlgPctOf = (c) => c && c.total_words > 0 ? (c.dialogue_words / c.total_words) * 100 : 0;

    const NAR_GREEN = '#2d6a5e';
    const DLG_PURPLE = '#5a3a82';

    for (let i = 0; i < slotCount; i++) {
      const groupX = 60 + i * (groupW + groupGap);
      const own = data[i];
      const comp = hasComp ? compareBook.chapters[i] : null;

      // Own narrative
      if (own) {
        const narH = (narPctOf(own) / maxVal) * chartH;
        ctx.fillStyle = NAR_GREEN;
        ctx.fillRect(groupX, chartH - narH + 15, singleBarW, narH);
      }
      // Own dialogue
      if (own) {
        const dlgH = (dlgPctOf(own) / maxVal) * chartH;
        ctx.fillStyle = DLG_PURPLE;
        ctx.fillRect(groupX + singleBarW + pairGap, chartH - dlgH + 15, singleBarW, dlgH);
      }
      // Comp narrative (dark orange)
      if (comp) {
        const offset = hasComp ? 2 : 0;
        const narH = (narPctOf(comp) / maxVal) * chartH;
        ctx.fillStyle = ORANGE_DARK;
        ctx.fillRect(groupX + (singleBarW + pairGap) * offset, chartH - narH + 15, singleBarW, narH);
      }
      // Comp dialogue (light orange)
      if (comp) {
        const offset = hasComp ? 3 : 0;
        const dlgH = (dlgPctOf(comp) / maxVal) * chartH;
        ctx.fillStyle = ORANGE_LIGHT;
        ctx.fillRect(groupX + (singleBarW + pairGap) * offset, chartH - dlgH + 15, singleBarW, dlgH);
      }

      // Percentages below
      ctx.font = '9px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      if (own) {
        const total = own.total_words || 1;
        ctx.fillStyle = NAR_GREEN;
        ctx.fillText(`${Math.round(own.narrative_words / total * 100)}%`, groupX + singleBarW / 2, chartH + 26);
        ctx.fillStyle = DLG_PURPLE;
        ctx.fillText(`${Math.round(own.dialogue_words / total * 100)}%`, groupX + singleBarW + pairGap + singleBarW / 2, chartH + 26);
      }
      if (comp) {
        const total = comp.total_words || 1;
        ctx.fillStyle = ORANGE_DARK;
        ctx.fillText(`${Math.round(comp.narrative_words / total * 100)}%`, groupX + (singleBarW + pairGap) * 2 + singleBarW / 2, chartH + 26);
        ctx.fillStyle = ORANGE_LIGHT;
        ctx.fillText(`${Math.round(comp.dialogue_words / total * 100)}%`, groupX + (singleBarW + pairGap) * 3 + singleBarW / 2, chartH + 26);
      }

      // Chapter name
      const titleSource = own ? own.chapter_title : (comp ? comp.chapter_title : `#${i + 1}`);
      ctx.save();
      ctx.translate(groupX + groupW / 2, chartH + 38);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = 'right';
      ctx.font = '11px Source Sans 3, system-ui';
      ctx.fillStyle = own ? '#9e9891' : ORANGE_DARK;
      const label = titleSource.length > 22 ? titleSource.slice(0, 22) + '…' : titleSource;
      ctx.fillText(label, 0, 0);
      ctx.restore();
    }

    // Legend
    let lx = logicalW - (hasComp ? 480 : 180);
    const drawLegend = (color, text) => {
      ctx.fillStyle = color; ctx.fillRect(lx, 8, 12, 12);
      ctx.fillStyle = '#6b6560'; ctx.font = '11px Source Sans 3, system-ui'; ctx.textAlign = 'left';
      ctx.fillText(text, lx + 16, 18);
      lx += ctx.measureText(text).width + 30;
    };
    drawLegend(NAR_GREEN, 'Narrative');
    drawLegend(DLG_PURPLE, 'Dialogue');
    if (hasComp) {
      drawLegend(ORANGE_DARK, `${compareBook.title} narrative`);
      drawLegend(ORANGE_LIGHT, `${compareBook.title} dialogue`);
    }
  }

  function drawSentenceLenChart() {
    drawBarChart(
      sentenceLenCanvas,
      data.map(d => Math.round(d.avg_sentence_length * 10) / 10),
      '#2d6a5e',
      compareValues(c => Math.round(c.avg_sentence_length * 10) / 10)
    );
  }

  function drawVocabChart() {
    drawBarChart(
      vocabCanvas,
      data.map(d => Math.round(d.vocabulary_density * 100)),
      '#2d6a5e',
      compareValues(c => Math.round(c.vocabulary_density * 100))
    );
  }
</script>

<div class="chap-page">
  <div class="chap-toolbar">
    <h1 class="chap-title">Chapter Analysis</h1>
    <div class="chap-wpm">
      <label>
        Reading speed:
        <input type="number" bind:value={wordsPerMin} min="100" max="600" class="wpm-input" />
        <span class="wpm-unit">wpm</span>
      </label>
      {#if compareBook}
        <label class="cmp-toggle">
          <input type="checkbox" bind:checked={showComparison} />
          Compare vs <strong>{compareBook.title}</strong>
        </label>
      {/if}
    </div>
  </div>

  {#if loading}
    <div class="chap-loading">Analysing manuscript...</div>
  {:else if !data || data.length === 0}
    <div class="chap-loading">No chapters to analyse.</div>
  {:else}
    {@const totalWords = data.reduce((s, c) => s + c.total_words, 0)}
    {@const totalDialogue = data.reduce((s, c) => s + c.dialogue_words, 0)}
    {@const readMins = Math.ceil(totalWords / wordsPerMin)}
    {@const readHrs = Math.floor(readMins / 60)}

    <div class="chap-content">
      <!-- Summary -->
      <div class="summary-grid">
        <div class="summary-card">
          <span class="summary-val">{totalWords.toLocaleString()}</span>
          <span class="summary-label">Total Words</span>
        </div>
        <div class="summary-card">
          <span class="summary-val">{readHrs > 0 ? `${readHrs}h ${readMins % 60}m` : `${readMins}m`}</span>
          <span class="summary-label">Reading Time</span>
        </div>
        <div class="summary-card">
          <span class="summary-val">{data.length}</span>
          <span class="summary-label">Chapters</span>
        </div>
        <div class="summary-card">
          <span class="summary-val">{Math.round(totalDialogue / totalWords * 100)}%</span>
          <span class="summary-label">Dialogue</span>
        </div>
        <div class="summary-card">
          <span class="summary-val">{Math.round(totalWords / data.length).toLocaleString()}</span>
          <span class="summary-label">Avg Words/Chapter</span>
        </div>
        <div class="summary-card">
          <span class="summary-val">{(data.reduce((s, c) => s + c.avg_sentence_length, 0) / data.length).toFixed(1)}</span>
          <span class="summary-label">Avg Sentence Length</span>
        </div>
      </div>

      <!-- Charts -->
      <div class="chart-section">
        <h2 class="chart-title">Words per Chapter</h2>
        <div class="chart-wrap"><canvas bind:this={chapCanvas}></canvas></div>
      </div>

      <div class="chart-section">
        <h2 class="chart-title">Narrative vs Dialogue</h2>
        <div class="chart-wrap"><canvas bind:this={dialogueCanvas}></canvas></div>
      </div>

      <div class="chart-section">
        <h2 class="chart-title">Avg Sentence Length</h2>
        <p class="chart-hint">Longer sentences = denser prose. Short = punchy/action.</p>
        <div class="chart-wrap"><canvas bind:this={sentenceLenCanvas}></canvas></div>
      </div>

      <div class="chart-section">
        <h2 class="chart-title">Vocabulary Density</h2>
        <p class="chart-hint">% of unique words. Lower = more repetitive language.</p>
        <div class="chart-wrap"><canvas bind:this={vocabCanvas}></canvas></div>
      </div>

      <!-- Detail table -->
      <div class="chart-section">
        <h2 class="chart-title">Chapter Breakdown</h2>
        <div class="table-wrap">
          <table class="detail-table">
            <thead>
              <tr>
                <th>Chapter</th>
                <th>Words</th>
                <th>Read Time</th>
                <th>Dialogue</th>
                <th>Narrative</th>
                <th>Dlg %</th>
                <th>Sentences</th>
                <th>Avg Sent Len</th>
                <th>Longest Sent</th>
                <th>Paragraphs</th>
                <th>Unique Words</th>
                <th>Vocab %</th>
              </tr>
            </thead>
            <tbody>
              {#each data as ch}
                <tr>
                  <td class="td-name">{ch.chapter_title}</td>
                  <td>{ch.total_words.toLocaleString()}</td>
                  <td>{Math.ceil(ch.total_words / wordsPerMin)}m</td>
                  <td>{ch.dialogue_words.toLocaleString()}</td>
                  <td>{ch.narrative_words.toLocaleString()}</td>
                  <td>{ch.total_words > 0 ? Math.round(ch.dialogue_words / ch.total_words * 100) : 0}%</td>
                  <td>{ch.sentence_count}</td>
                  <td>{ch.avg_sentence_length.toFixed(1)}</td>
                  <td>{ch.longest_sentence}</td>
                  <td>{ch.paragraph_count}</td>
                  <td>{ch.unique_words.toLocaleString()}</td>
                  <td>{(ch.vocabulary_density * 100).toFixed(0)}%</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    overflow: auto !important;
    height: auto !important;
  }
  .chap-page {
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #faf8f5; min-height: 100vh;
  }
  .chap-toolbar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1rem 1.5rem; border-bottom: 1px solid #e5e1da; background: white;
  }
  .chap-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.2rem; font-weight: 400; margin: 0; color: #2d2a26;
  }
  .chap-wpm { display: flex; align-items: center; gap: 1rem; font-size: 0.85rem; color: #6b6560; flex-wrap: wrap; }
  .cmp-toggle {
    display: inline-flex; align-items: center; gap: 0.45rem;
    cursor: pointer; padding-left: 0.8rem; border-left: 1px solid #e5e1da;
    white-space: nowrap;
  }
  .cmp-toggle input { accent-color: #a85a04; width: 16px; height: 16px; }
  .cmp-toggle strong { color: #a85a04; font-weight: 600; }
  .wpm-input {
    width: 65px; padding: 0.3rem 0.4rem; border: 1px solid #e5e1da;
    border-radius: 4px; font-size: 0.85rem; text-align: center;
  }
  .wpm-unit { font-size: 0.75rem; color: #9e9891; }

  .chap-loading {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: #9e9891; font-style: italic;
  }
  .chap-content { flex: 1; overflow: auto; padding: 1.5rem; }

  .summary-grid {
    display: grid; grid-template-columns: repeat(3, 1fr); gap: 1px;
    background: #e5e1da; border-radius: 8px; overflow: hidden; margin-bottom: 1.5rem;
  }
  .summary-card {
    display: flex; flex-direction: column; align-items: center;
    padding: 1rem; background: white;
  }
  .summary-val {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.5rem; font-weight: 600; color: #2d2a26;
  }
  .summary-label {
    font-size: 0.7rem; color: #9e9891; text-transform: uppercase;
    letter-spacing: 0.06em; margin-top: 0.2rem;
  }

  .chart-section { margin-bottom: 1.5rem; }
  .chart-section.half { flex: 1; min-width: 0; }
  .chart-row { display: flex; gap: 1.5rem; }
  .chart-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 400; margin: 0 0 0.3rem; color: #2d2a26;
  }
  .chart-hint { font-size: 0.75rem; color: #9e9891; margin: 0 0 0.5rem; font-style: italic; }
  .chart-wrap {
    overflow-x: auto; border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.5rem;
  }
  .chart-wrap canvas { display: block; }

  .table-wrap { overflow-x: auto; }
  .detail-table {
    width: 100%; border-collapse: collapse; font-size: 0.8rem;
  }
  .detail-table th {
    text-align: left; padding: 0.5rem; font-weight: 600; color: #6b6560;
    border-bottom: 2px solid #e5e1da; font-size: 0.7rem;
    text-transform: uppercase; letter-spacing: 0.04em; white-space: nowrap;
  }
  .detail-table td {
    padding: 0.4rem 0.5rem; border-bottom: 1px solid #eeeae4;
    color: #6b6560; white-space: nowrap;
  }
  .td-name { font-weight: 500; color: #2d2a26; }
  .detail-table tbody tr:hover { background: #f0ede8; }
</style>
