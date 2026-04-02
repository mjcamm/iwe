<script>
  import { onMount } from 'svelte';
  import { chapterAnalysis } from '$lib/db.js';

  let data = $state(null);
  let loading = $state(true);
  let wordsPerMin = $state(250);

  let chapCanvas;
  let dialogueCanvas;
  let sentenceLenCanvas;
  let vocabCanvas;

  onMount(async () => {
    try {
      data = await chapterAnalysis();
    } catch (e) {
      console.warn('Chapter analysis failed:', e);
    }
    loading = false;
  });

  $effect(() => { if (data && chapCanvas) drawWordCountChart(); });
  $effect(() => { if (data && dialogueCanvas) drawDialogueChart(); });
  $effect(() => { if (data && sentenceLenCanvas) drawSentenceLenChart(); });
  $effect(() => { if (data && vocabCanvas) drawVocabChart(); });

  function drawBarChart(canvas, values, color) {
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 600;
    const barW = Math.max(30, Math.min(60, (containerW - 80) / data.length));
    const chartH = 220;
    const labelH = 70;
    const logicalW = Math.max(containerW, 70 + data.length * (barW + 6));
    const logicalH = chartH + labelH + 30;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    const maxVal = Math.max(...values, 1);

    for (let i = 0; i < data.length; i++) {
      const val = values[i];
      const h = (val / maxVal) * chartH;
      const x = 60 + i * (barW + 6);
      const y = chartH - h + 15;

      ctx.fillStyle = color;
      ctx.fillRect(x, y, barW, h);
      ctx.fillStyle = '#6b6560';
      ctx.font = '11px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      ctx.fillText(typeof val === 'number' && val % 1 !== 0 ? val.toFixed(1) : val.toLocaleString(), x + barW / 2, y - 5);

      ctx.save();
      ctx.translate(x + barW / 2, chartH + 25);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = 'right';
      ctx.fillStyle = '#9e9891';
      ctx.fillText(data[i].chapter_title, 0, 0);
      ctx.restore();
    }
  }

  function drawWordCountChart() {
    drawBarChart(chapCanvas, data.map(d => d.total_words), '#2d6a5e');
  }

  function drawDialogueChart() {
    const ctx = dialogueCanvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = dialogueCanvas.parentElement?.clientWidth || 600;
    const singleBarW = Math.max(14, Math.min(28, (containerW - 100) / (data.length * 2.5)));
    const pairGap = 2; // gap between dialogue & narrative bars
    const groupGap = singleBarW * 1.2; // gap between chapter groups
    const pairW = singleBarW * 2 + pairGap;
    const chartH = 220;
    const labelH = 90;
    const logicalW = Math.max(containerW, 80 + data.length * (pairW + groupGap));
    const logicalH = chartH + labelH + 30;

    dialogueCanvas.width = logicalW * dpr;
    dialogueCanvas.height = logicalH * dpr;
    dialogueCanvas.style.width = logicalW + 'px';
    dialogueCanvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    const maxVal = Math.max(...data.map(d => Math.max(d.narrative_words, d.dialogue_words)), 1);

    for (let i = 0; i < data.length; i++) {
      const groupX = 60 + i * (pairW + groupGap);
      const narH = (data[i].narrative_words / maxVal) * chartH;
      const dlgH = (data[i].dialogue_words / maxVal) * chartH;

      // Narrative bar
      ctx.fillStyle = '#2d6a5e';
      ctx.fillRect(groupX, chartH - narH + 15, singleBarW, narH);

      // Dialogue bar
      ctx.fillStyle = '#d97706';
      ctx.fillRect(groupX + singleBarW + pairGap, chartH - dlgH + 15, singleBarW, dlgH);

      // Percentages below bars
      const total = data[i].total_words || 1;
      const narPct = Math.round(data[i].narrative_words / total * 100);
      const dlgPct = Math.round(data[i].dialogue_words / total * 100);

      ctx.font = '9px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      ctx.fillStyle = '#2d6a5e';
      ctx.fillText(`${narPct}%`, groupX + singleBarW / 2, chartH + 26);
      ctx.fillStyle = '#d97706';
      ctx.fillText(`${dlgPct}%`, groupX + singleBarW + pairGap + singleBarW / 2, chartH + 26);

      // Chapter name
      ctx.save();
      ctx.translate(groupX + pairW / 2, chartH + 38);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = 'right';
      ctx.font = '11px Source Sans 3, system-ui';
      ctx.fillStyle = '#9e9891';
      ctx.fillText(data[i].chapter_title, 0, 0);
      ctx.restore();
    }

    // Legend
    ctx.fillStyle = '#2d6a5e'; ctx.fillRect(logicalW - 180, 8, 12, 12);
    ctx.fillStyle = '#6b6560'; ctx.font = '11px Source Sans 3, system-ui'; ctx.textAlign = 'left';
    ctx.fillText('Narrative', logicalW - 164, 18);
    ctx.fillStyle = '#d97706'; ctx.fillRect(logicalW - 90, 8, 12, 12);
    ctx.fillStyle = '#6b6560'; ctx.fillText('Dialogue', logicalW - 74, 18);
  }

  function drawSentenceLenChart() {
    drawBarChart(sentenceLenCanvas, data.map(d => Math.round(d.avg_sentence_length * 10) / 10), '#6a4c2d');
  }

  function drawVocabChart() {
    drawBarChart(vocabCanvas, data.map(d => Math.round(d.vocabulary_density * 100)), '#4c2d6a');
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
        <h2 class="chart-title">Dialogue vs Narrative</h2>
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
  .chap-wpm { display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; color: #6b6560; }
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
