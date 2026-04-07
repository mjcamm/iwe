<script>
  import { onMount, tick } from 'svelte';
  import { page } from '$app/stores';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    openProject,
    chapterAnalysis,
    pacingAnalysis,
    readabilityAnalysis,
    paragraphLengthAnalysis,
    adverbAnalysis,
    wordFrequency,
    getAllChapterWordCounts,
    saveLibraryBook
  } from '$lib/db.js';
  import { invoke } from '@tauri-apps/api/core';

  // closeProject isn't in db.js — call it directly
  async function closeProjectSafe() {
    try { await invoke('close_project'); } catch {}
  }

  // The list of analyses we run, in order. Each entry: { key, label, fn }
  // These are the EXACT same commands the project analysis panel uses.
  const STEPS = [
    { key: 'chapter',    label: 'Chapter analysis',         fn: () => chapterAnalysis() },
    // Pacing: strip sentence_texts and sentence_starts — we only need the
    // sentence_lengths array for whole-book waveform comparison.
    { key: 'pacing',     label: 'Pacing (sentence length)', fn: async () => {
      const raw = await pacingAnalysis();
      if (Array.isArray(raw)) {
        return raw.map(ch => ({
          chapter_id: ch.chapter_id,
          chapter_title: ch.chapter_title,
          sentence_lengths: ch.sentence_lengths
        }));
      }
      return raw;
    }},
    // Readability: strip sentence_texts/sentence_grades/sentence_starts —
    // comparison only uses per-chapter total_words + grade_level.
    { key: 'readability',label: 'Readability (FK grade)',   fn: async () => {
      const raw = await readabilityAnalysis();
      if (raw && Array.isArray(raw.chapters)) {
        return {
          ...raw,
          chapters: raw.chapters.map(ch => ({
            chapter_id: ch.chapter_id,
            chapter_title: ch.chapter_title,
            grade_level: ch.grade_level,
            total_words: ch.total_words,
            total_sentences: ch.total_sentences,
            total_syllables: ch.total_syllables,
            avg_words_per_sentence: ch.avg_words_per_sentence
          }))
        };
      }
      return raw;
    }},
    // Paragraphs: strip per-paragraph previews and char positions — keep
    // word counts so the per-chapter overlay still works.
    { key: 'paragraphs', label: 'Paragraph length',         fn: async () => {
      const raw = await paragraphLengthAnalysis();
      if (raw && Array.isArray(raw.chapters)) {
        return {
          ...raw,
          chapters: raw.chapters.map(ch => ({
            chapter_id: ch.chapter_id,
            chapter_title: ch.chapter_title,
            total_paragraphs: ch.total_paragraphs,
            avg_length: ch.avg_length,
            variation_pct: ch.variation_pct,
            paragraphs: (ch.paragraphs || []).map(p => ({ word_count: p.word_count }))
          }))
        };
      }
      return raw;
    }},
    // Adverbs: strip per-instance dialogue snippets and contexts — keep
    // headline counts and top_adverbs which are all the comparison needs.
    { key: 'adverbs',    label: 'Adverbs in dialogue',      fn: async () => {
      const raw = await adverbAnalysis();
      if (raw) {
        return {
          total_dialogue_spans: raw.total_dialogue_spans,
          attributions_with_adverbs: raw.attributions_with_adverbs,
          total_instances: raw.total_instances,
          redundant_count: raw.redundant_count,
          top_adverbs: raw.top_adverbs
        };
      }
      return raw;
    }},
    // Word frequency: store loose filters (min length 1, min count 10) so the
    // saved blob covers most useful comparison cases. Strip per-chapter detail
    // to keep the JSON blob small — comparative views only need {word, count}.
    { key: 'frequency',  label: 'Word frequency',           fn: async () => {
      const raw = await wordFrequency(1, 5, null);
      if (Array.isArray(raw)) {
        return raw.map(w => ({ word: w.word, total_count: w.total_count }));
      }
      return raw;
    }}
  ];

  let filepath = $state('');
  let title = $state('');
  let author = $state('');

  let progress = $state(0);
  let currentLabel = $state('');
  let phase = $state('idle'); // idle | running | done | error
  let errorMsg = $state('');
  let results = $state({}); // key → result
  let totalWords = $state(0);

  onMount(async () => {
    const params = $page.url.searchParams;
    filepath = params.get('filepath') || '';
    if (!filepath) {
      phase = 'error';
      errorMsg = 'No project file specified';
      return;
    }
    title = (filepath.split(/[\\/]/).pop() || 'Untitled').replace(/\.iwe$/, '');
    await runAll();
  });

  async function runAll() {
    phase = 'running';
    progress = 0;
    results = {};
    try {
      currentLabel = 'Opening project…';
      await tick();
      await openProject(filepath);

      // Total word count for the saved record (Rust returns Vec<(i64, usize)>)
      try {
        const counts = await getAllChapterWordCounts();
        totalWords = (counts || []).reduce((acc, entry) => acc + (entry[1] || 0), 0);
      } catch {}

      for (let i = 0; i < STEPS.length; i++) {
        const step = STEPS[i];
        currentLabel = step.label;
        progress = i;
        await tick();
        try {
          results[step.key] = await step.fn();
        } catch (e) {
          results[step.key] = { error: String(e) };
        }
        progress = i + 1;
        await tick();
      }
      phase = 'done';
      currentLabel = '';
    } catch (e) {
      phase = 'error';
      errorMsg = String(e);
    } finally {
      await closeProjectSafe();
    }
  }

  async function saveToLibrary() {
    try {
      const json = JSON.stringify(results);
      const id = await saveLibraryBook(title, author || '', filepath.split(/[\\/]/).pop() || '', totalWords, json);
      alert(`Saved "${title}" to library (id ${id}).`);
    } catch (e) {
      alert('Save failed: ' + e);
    }
  }

  async function close() {
    await getCurrentWindow().close();
  }

  // ---- Display helpers ----
  function fmt(n, digits = 1) {
    if (n == null || Number.isNaN(n)) return '—';
    if (typeof n !== 'number') return String(n);
    if (Number.isInteger(n)) return n.toLocaleString();
    return n.toFixed(digits);
  }

  let summary = $derived.by(() => {
    if (phase !== 'done') return null;
    const out = [];
    const ch = results.chapter;
    if (ch && Array.isArray(ch)) {
      const wcs = ch.map(c => c.total_words ?? c.totalWords ?? 0);
      const total = wcs.reduce((a, b) => a + b, 0);
      if (total > totalWords) totalWords = total;
      out.push({ label: 'Chapters', value: fmt(ch.length) });
      out.push({ label: 'Total words', value: fmt(total) });
      out.push({ label: 'Avg chapter', value: fmt(total / Math.max(1, ch.length), 0) + ' wd' });
    }
    const pacing = results.pacing;
    if (pacing && pacing.overall_mean !== undefined) {
      out.push({ label: 'Avg sentence', value: fmt(pacing.overall_mean, 1) + ' wd' });
    } else if (pacing && pacing.overallMean !== undefined) {
      out.push({ label: 'Avg sentence', value: fmt(pacing.overallMean, 1) + ' wd' });
    }
    const read = results.readability;
    if (read && (read.overall_grade !== undefined || read.overallGrade !== undefined)) {
      out.push({ label: 'FK grade', value: fmt(read.overall_grade ?? read.overallGrade, 1) });
    }
    const para = results.paragraphs;
    if (para && (para.overall_mean !== undefined || para.overallMean !== undefined)) {
      out.push({ label: 'Avg paragraph', value: fmt(para.overall_mean ?? para.overallMean, 1) + ' wd' });
    }
    const adv = results.adverbs;
    if (adv && Array.isArray(adv)) {
      out.push({ label: 'Adverbs flagged', value: fmt(adv.length) });
    }
    const freq = results.frequency;
    if (freq && Array.isArray(freq)) {
      out.push({ label: 'Repeated words', value: fmt(freq.length) });
    }
    return out;
  });
</script>

<svelte:head><title>Analyse — {title}</title></svelte:head>

<div class="wrap">
  <header class="topbar">
    <div class="title-block">
      <input class="title-input" type="text" bind:value={title} placeholder="Book title…" />
      <input class="author-input" type="text" bind:value={author} placeholder="Author (optional)" />
    </div>
    <div class="actions">
      {#if phase === 'done'}
        <button class="btn primary" onclick={saveToLibrary}>Save to library</button>
      {/if}
      <button class="btn ghost" onclick={close}>Close</button>
    </div>
  </header>

  {#if phase === 'running' || phase === 'idle'}
    <div class="loading">
      <div class="loading-card">
        <div class="loading-title">Running analysis</div>
        <div class="loading-sub">{progress} of {STEPS.length} steps</div>
        <div class="bar"><div class="bar-fill" style="width: {(progress / STEPS.length) * 100}%"></div></div>
        <div class="current">{currentLabel || ' '}</div>
      </div>
    </div>
  {:else if phase === 'error'}
    <div class="err">{errorMsg}</div>
  {:else}
    <div class="results">
      <section class="summary">
        {#each summary as s}
          <div class="metric">
            <div class="m-label">{s.label}</div>
            <div class="m-value">{s.value}</div>
          </div>
        {/each}
      </section>

      <section class="raw">
        <h2 class="raw-h">Raw analysis output</h2>
        {#each STEPS as step}
          <details class="raw-block">
            <summary>{step.label} ({step.key})</summary>
            <pre>{JSON.stringify(results[step.key], null, 2)}</pre>
          </details>
        {/each}
      </section>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    overflow: auto !important; height: auto !important;
    margin: 0; background: #faf8f5;
    font-family: 'Source Sans 3', system-ui, sans-serif;
    color: #2d2a26;
  }
  .wrap { min-height: 100vh; display: flex; flex-direction: column; }
  .topbar {
    position: sticky; top: 0; z-index: 10;
    display: flex; align-items: center; gap: 1rem;
    padding: 0.8rem 1.4rem;
    background: #fffef9;
    border-bottom: 1px solid #e6e0d6;
  }
  .title-block { flex: 1; display: flex; flex-direction: column; gap: 0.3rem; min-width: 0; }
  .title-input {
    font-family: 'Libre Baskerville', serif;
    font-size: 1.2rem; font-weight: 400; color: #2d2a26;
    border: none; border-bottom: 1px dashed transparent;
    background: transparent; padding: 0.2rem 0.3rem; max-width: 480px;
  }
  .title-input:focus { outline: none; border-bottom-color: #2d6a5e; }
  .author-input {
    border: none; border-bottom: 1px dashed transparent;
    background: transparent; font-size: 0.85rem;
    color: #6b6560; padding: 0.15rem 0; max-width: 280px;
    font-family: inherit;
  }
  .author-input:focus { outline: none; border-bottom-color: #2d6a5e; }
  .actions { display: flex; gap: 0.5rem; }
  .btn {
    font-family: inherit; font-size: 0.85rem;
    padding: 0.45rem 1rem; border: none; border-radius: 4px;
    cursor: pointer;
  }
  .btn.primary { background: #2d6a5e; color: white; }
  .btn.primary:hover { background: #245449; }
  .btn.ghost { background: transparent; color: #6b6560; }
  .btn.ghost:hover { background: #f0ebe0; }

  .loading { display: flex; align-items: center; justify-content: center; padding: 5rem 2rem; }
  .loading-card {
    background: #fffef9; border-radius: 8px;
    padding: 1.8rem 2.2rem; width: 460px; max-width: 90vw;
    box-shadow: 0 8px 30px rgba(0,0,0,0.08);
    text-align: center;
  }
  .loading-title { font-family: 'Libre Baskerville', serif; font-size: 1.05rem; }
  .loading-sub { font-size: 0.78rem; color: #6b6560; margin: 0.3rem 0 1rem; font-variant-numeric: tabular-nums; }
  .bar { height: 6px; background: #f0ebe0; border-radius: 3px; overflow: hidden; }
  .bar-fill { height: 100%; background: #2d6a5e; transition: width 200ms ease; }
  .current { font-size: 0.78rem; color: #6b6560; font-style: italic; min-height: 1.1em; margin-top: 0.7rem; }

  .err { padding: 4rem; text-align: center; color: #a0403d; font-family: 'Libre Baskerville', serif; }

  .results { padding: 2rem 2rem 4rem; max-width: 1100px; margin: 0 auto; width: 100%; box-sizing: border-box; }
  .summary {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 0.75rem; margin-bottom: 2rem;
  }
  .metric {
    background: #fffef9; border: 1px solid #e6e0d6; border-radius: 6px;
    padding: 1rem 1.2rem;
  }
  .m-label { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.06em; color: #6b6560; }
  .m-value { font-family: 'Libre Baskerville', serif; font-size: 1.5rem; color: #2d6a5e; margin-top: 0.3rem; }

  .raw-h {
    font-family: 'Libre Baskerville', serif; font-weight: 400;
    font-size: 1rem; color: #2d2a26; margin: 1rem 0 0.6rem;
    border-bottom: 1px solid #e6e0d6; padding-bottom: 0.4rem;
  }
  .raw-block {
    background: #fffef9; border: 1px solid #e6e0d6; border-radius: 5px;
    margin-bottom: 0.5rem;
  }
  .raw-block summary {
    cursor: pointer; padding: 0.6rem 0.9rem; font-size: 0.85rem;
    color: #2d2a26;
  }
  .raw-block pre {
    font-size: 0.72rem; padding: 0.6rem 0.9rem;
    margin: 0; max-height: 400px; overflow: auto;
    background: #f7f3eb; border-top: 1px solid #e6e0d6;
    color: #3a3530;
  }
</style>
