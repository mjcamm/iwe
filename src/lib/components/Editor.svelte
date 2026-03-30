<script>
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Placeholder from '@tiptap/extension-placeholder';
  import TextAlign from '@tiptap/extension-text-align';
  import Superscript from '@tiptap/extension-superscript';
  import Subscript from '@tiptap/extension-subscript';
  import { Extension } from '@tiptap/core';
  import { scanText, checkWord, addIgnoredWord, checkSpelling, addToDictionary } from '$lib/db.js';
  import { createHighlightPlugin, createSpellCheckPlugin, buildTextMap, applyDecorations, applySpellDecorations, entityHighlightKey, spellCheckKey } from '$lib/entityHighlight.js';
  import WordModal from './WordModal.svelte';

  let { openTabs, activeTabId, chapter, onselecttab, onclosetab, onchange, onselectionchange, onentityclick, onquickadd, viewedEntityIds = new Set() } = $props();

  let element = $state();
  // Official Svelte 5 pattern: wrap editor in object so reassignment triggers reactivity
  let editorState = $state({ editor: null });
  let updatingFromProp = false;
  let showStyleDropdown = $state(false);

  // Scan state — lives in Svelte, not in TipTap
  let scanMatches = $state([]);
  let scanPosMap = $state([]);
  let scanTimer = null;
  let applyingDecorations = false;

  // Spell check state
  let spellErrors = $state([]); // [{ word, start, end, suggestions }]
  let spellTimer = null;

  // Context menu state
  let ctxMenu = $state({ show: false, x: 0, y: 0, word: '', from: 0, to: 0, isEntity: false, entityId: null, entityName: null, isMisspelled: false, suggestions: [] });

  // Word modal state
  let wordModal = $state({ show: false, word: '', isMisspelled: false, suggestions: [], from: 0, to: 0 });

  // Live suggestion bubbles — multiple at once, each with own timer
  let suggestions = $state([]); // [{ word, top, left, startTime, id }]
  const SUGGESTION_DURATION = 30000;
  let keystrokeCount = 0;
  let suggestionIdCounter = 0;
  let suggestionTick = null;

  function tickSuggestions() {
    const now = Date.now();
    const remaining = suggestions.filter(s => now - s.startTime < SUGGESTION_DURATION);
    // Reassign to trigger Svelte re-render (updates progress bars)
    suggestions = remaining;
    if (suggestions.length > 0) {
      suggestionTick = setTimeout(tickSuggestions, 500);
    } else {
      suggestionTick = null;
    }
  }

  function addSuggestion(word, top, left) {
    if (suggestions.find(s => s.word === word)) return;
    suggestions = [...suggestions, { word, top, left, startTime: Date.now(), id: ++suggestionIdCounter }];
    if (!suggestionTick) suggestionTick = setTimeout(tickSuggestions, 500);
  }

  function dismissSuggestion(id) {
    suggestions = suggestions.filter(s => s.id !== id);
  }

  async function handleSuggestionAdd(s) {
    dismissSuggestion(s.id);
    onquickadd?.(s.word);
  }

  async function handleSuggestionIgnore(s) {
    dismissSuggestion(s.id);
    await addIgnoredWord(s.word);
  }

  function checkForSuggestion() {
    if (!editorRaw) return;
    const { state } = editorRaw;
    const { from } = state.selection;

    const start = Math.max(1, from - 150);
    let recentText;
    try {
      recentText = state.doc.textBetween(start, from, ' ');
    } catch { return; }
    if (!recentText) return;

    // Only match completed words — must be followed by a non-letter character (space, punctuation, etc.)
    const words = [...recentText.matchAll(/[A-Z\u00C0-\u024F][a-zA-Z\u00C0-\u024F'\u2019]*(?=[^a-zA-Z\u00C0-\u024F])/g)];
    if (words.length === 0) return;

    // Check ALL candidate words, not just the most recent
    for (const m of words) {
      const word = m[0];
      const wordIdx = m.index;

      if (word.length < 2) continue;
      // Already showing?
      if (suggestions.find(s => s.word === word)) continue;

      // Sentence start check
      const before = recentText.slice(0, wordIdx);
      let foundTerminator = false;
      let foundLetter = false;
      for (let i = before.length - 1; i >= 0; i--) {
        const ch = before[i];
        if ('.?!:\u2014'.includes(ch)) { foundTerminator = true; break; }
        if (/[a-zA-Z0-9]/.test(ch)) { foundLetter = true; break; }
      }
      if (!before.trim()) foundTerminator = true;
      if (foundTerminator || (!foundLetter && !foundTerminator)) continue;

      // Check with Rust (async, fire and forget for each word)
      const capturedWord = word;
      checkWord(capturedWord).then(isCandidate => {
        if (!isCandidate || !editorRaw) return;
        if (suggestions.find(s => s.word === capturedWord)) return;

        // Find word position in doc
        const doc = editorRaw.state.doc;
        let wordFrom = null;
        doc.descendants((node, pos) => {
          if (wordFrom !== null || !node.isText) return;
          const i = node.text.indexOf(capturedWord);
          if (i >= 0 && Math.abs(pos + i - from) < 200) {
            wordFrom = pos + i;
          }
        });

        const posForBubble = wordFrom !== null ? wordFrom : from;
        const coords = editorRaw.view.coordsAtPos(posForBubble);
        const scrollEl = element.closest('.editor-scroll');
        if (!scrollEl) return;
        const scrollRect = scrollEl.getBoundingClientRect();

        // Position above the word — bubble is ~75px tall, add 10px gap
        let top = coords.top - scrollRect.top + scrollEl.scrollTop - 85;

        // Avoid overlapping with existing bubbles — nudge up if too close
        for (const existing of suggestions) {
          if (Math.abs(existing.top - top) < 70 && Math.abs(existing.left - (coords.left - scrollRect.left)) < 120) {
            top = existing.top - 70;
          }
        }

        const left = Math.max(10, Math.min(coords.left - scrollRect.left, scrollRect.width - 120));

        addSuggestion(capturedWord, top, left);
      }).catch(() => {});
    }
  }

  // ---- Context menu ----

  function getWordAtPos(view, pos) {
    const doc = view.state.doc;
    const wordPattern = /[a-zA-Z\u00C0-\u024F'\u2019]/;

    // Walk backward from pos to find word start
    let from = pos;
    while (from > 0) {
      const prevChar = doc.textBetween(from - 1, from, '', '');
      if (!prevChar || !wordPattern.test(prevChar)) break;
      from--;
    }

    // Walk forward from pos to find word end
    let to = pos;
    const docSize = doc.content.size;
    while (to < docSize) {
      const nextChar = doc.textBetween(to, to + 1, '', '');
      if (!nextChar || !wordPattern.test(nextChar)) break;
      to++;
    }

    if (from === to) return null;
    const rawWord = doc.textBetween(from, to, '', '');
    const word = rawWord.replace(/^['\u2019]+|['\u2019]+$/g, '');
    if (word.length < 2) return null;

    // Adjust from/to if we stripped leading/trailing apostrophes
    const leadStripped = rawWord.length - rawWord.replace(/^['\u2019]+/, '').length;
    const trailStripped = rawWord.length - rawWord.replace(/['\u2019]+$/, '').length;

    return { word, from: from + leadStripped, to: to - trailStripped };
  }

  function handleContextMenu(e) {
    if (!editorRaw) return;
    e.preventDefault();

    const coords = { left: e.clientX, top: e.clientY };
    const posResult = editorRaw.view.posAtCoords(coords);
    if (!posResult) return;

    const pos = posResult.pos;
    const wordInfo = getWordAtPos(editorRaw.view, pos);
    if (!wordInfo) return;

    // Check if this word is an entity
    const entityDecos = entityHighlightKey.getState(editorRaw.state);
    const entityFound = entityDecos ? entityDecos.find(wordInfo.from, wordInfo.to) : [];
    const entityDeco = entityFound.find(d => d.spec.entityId);

    // Check if this word is misspelled
    const spellDecos = spellCheckKey.getState(editorRaw.state);
    const spellFound = spellDecos ? spellDecos.find(wordInfo.from, wordInfo.to) : [];
    const isSpellError = spellFound.some(d => d.spec.spellError);

    // Find suggestions from spellErrors state
    let suggestions = [];
    if (isSpellError) {
      const err = spellErrors.find(e => e.word.toLowerCase() === wordInfo.word.toLowerCase());
      if (err) suggestions = err.suggestions;
    }

    ctxMenu = {
      show: true,
      x: e.clientX,
      y: e.clientY,
      word: wordInfo.word,
      from: wordInfo.from,
      to: wordInfo.to,
      isEntity: !!entityDeco,
      entityId: entityDeco?.spec.entityId || null,
      entityName: entityDeco?.spec.entityName || null,
      isMisspelled: isSpellError,
      suggestions,
    };
  }

  function closeCtxMenu() {
    ctxMenu = { ...ctxMenu, show: false };
  }

  function ctxOpenModal() {
    wordModal = {
      show: true,
      word: ctxMenu.word,
      isMisspelled: ctxMenu.isMisspelled,
      suggestions: ctxMenu.suggestions,
      from: ctxMenu.from,
      to: ctxMenu.to,
    };
    closeCtxMenu();
  }

  async function ctxAddToDictionary() {
    await addToDictionary(ctxMenu.word);
    closeCtxMenu();
    // Re-scan to remove squiggly
    doScan();
  }

  function ctxGoToDefinition() {
    if (ctxMenu.entityId) {
      onentityclick?.(ctxMenu.entityId, ctxMenu.entityName, true);
    }
    closeCtxMenu();
  }

  function ctxFindReferences() {
    if (ctxMenu.entityId) {
      onentityclick?.(ctxMenu.entityId, ctxMenu.entityName, false);
    }
    closeCtxMenu();
  }

  function ctxCreateEntity() {
    onquickadd?.(ctxMenu.word);
    closeCtxMenu();
  }

  async function ctxIgnore() {
    await addIgnoredWord(ctxMenu.word);
    closeCtxMenu();
    doScan();
  }

  function handleModalReplace(newWord, from, to) {
    if (!editorRaw) return;
    const { tr } = editorRaw.state;
    tr.replaceWith(from, to, editorRaw.state.schema.text(newWord));
    editorRaw.view.dispatch(tr);
    editorRaw.view.focus();
  }

  function handleModalDictionaryAdd() {
    doScan();
  }

  function handleModalClose() {
    wordModal = { ...wordModal, show: false };
  }

  function toHtml(content) {
    if (!content) return '';
    if (content.trim().startsWith('<')) return content;
    return content.split(/\n\n+/).map(p => `<p>${p.replace(/\n/g, '<br>')}</p>`).join('');
  }

  function doScan() {
    if (!editorRaw) return;
    const { text, posMap } = buildTextMap(editorRaw.state.doc);
    scanText(text).then(matches => {
      scanMatches = matches;
      scanPosMap = posMap;
      // Trigger spell check after entity scan
      doSpellCheck(text, posMap, matches);
    }).catch(e => { console.warn('[scan] error:', e); });
  }

  function doSpellCheck(text, posMap, entityMatches) {
    if (!text) return;

    // Build entity char ranges to exclude
    const entityChars = new Set();
    for (const m of entityMatches) {
      for (let i = m.start; i < m.end; i++) entityChars.add(i);
    }

    // Extract unique words with their positions
    const wordRegex = /[a-zA-Z\u00C0-\u024F'\u2019]+/g;
    let match;
    const uniqueWords = new Map(); // word -> [{ start, end }]
    while ((match = wordRegex.exec(text)) !== null) {
      const w = match[0];
      // Skip short words, all-apostrophe, or entity-overlapping
      if (w.length < 2) continue;
      const start = match.index;
      const end = start + w.length;
      let inEntity = false;
      for (let i = start; i < end; i++) {
        if (entityChars.has(i)) { inEntity = true; break; }
      }
      if (inEntity) continue;

      // Clean word: strip leading/trailing apostrophes
      const cleaned = w.replace(/^['\u2019]+|['\u2019]+$/g, '');
      if (cleaned.length < 2) continue;

      if (!uniqueWords.has(cleaned.toLowerCase())) {
        uniqueWords.set(cleaned.toLowerCase(), []);
      }
      uniqueWords.get(cleaned.toLowerCase()).push({ word: cleaned, start, end });
    }

    const wordsToCheck = [...uniqueWords.keys()];
    if (wordsToCheck.length === 0) {
      spellErrors = [];
      if (editorRaw) {
        applyingDecorations = true;
        applySpellDecorations(editorRaw, [], posMap, entityChars);
        applyingDecorations = false;
      }
      return;
    }

    checkSpelling(wordsToCheck).then(results => {
      // results is array of { word, suggestions } for misspelled words
      const errorMap = new Map();
      for (const r of results) {
        errorMap.set(r.word.toLowerCase(), r.suggestions);
      }

      // Build error positions
      const errors = [];
      for (const [lower, positions] of uniqueWords) {
        if (errorMap.has(lower)) {
          const sug = errorMap.get(lower);
          for (const pos of positions) {
            errors.push({ word: pos.word, start: pos.start, end: pos.end, suggestions: sug });
          }
        }
      }

      spellErrors = errors;

      // Apply spell decorations
      if (editorRaw) {
        applyingDecorations = true;
        applySpellDecorations(editorRaw, errors, posMap, entityChars);
        applyingDecorations = false;
      }
    }).catch(e => { console.warn('[spell] error:', e); });
  }

  // Exported so parent can trigger rescans after entity CRUD
  export function rescan() {
    doScan();
  }

  export function getEditor() {
    return editorRaw;
  }

  export function clearSelection() {
    if (!editorRaw) return;
    const { to } = editorRaw.state.selection;
    editorRaw.chain().setTextSelection(to).run();
  }

  onMount(() => {
    const ed = new Editor({
      element: element,
      extensions: [
        StarterKit.configure({
          heading: { levels: [1, 2, 3] }
        }),
        Placeholder.configure({
          placeholder: 'Begin writing...'
        }),
        TextAlign.configure({
          types: ['heading', 'paragraph']
        }),
        Superscript,
        Subscript,
        Extension.create({
          name: 'entityHighlightBridge',
          addProseMirrorPlugins() {
            return [
              createHighlightPlugin((entityId, entityName, isCtrl) => {
                onentityclick?.(entityId, entityName, isCtrl);
              }),
              createSpellCheckPlugin(),
            ];
          },
        }),
      ],
      content: toHtml(chapter?.content),
      editorProps: {
        attributes: {
          class: 'prose-editor',
          spellcheck: 'false'
        },
      },
      onUpdate: ({ editor: ed }) => {
        if (!updatingFromProp) {
          onchange(ed.getHTML());
        }
        // Check for live entity suggestion (debounced)
        // Check for entity suggestion every 4 keystrokes
        keystrokeCount++;
        if (keystrokeCount >= 4) {
          keystrokeCount = 0;
          checkForSuggestion();
        }
        // Debounced scan on content change
        clearTimeout(scanTimer);
        scanTimer = setTimeout(doScan, 400);
      },
      onTransaction: ({ editor }) => {
        // Don't trigger Svelte reactivity for decoration-only transactions
        if (!applyingDecorations) {
          editorState = { editor };
          const { from, to } = editor.state.selection;
          const text = from !== to ? editor.state.doc.textBetween(from, to, ' ') : '';
          onselectionchange?.(text);
        }
      },
    });
    editorRaw = ed;
    editorState = { editor: ed };

    // Initial scan
    setTimeout(doScan, 300);

    // Close dropdown on click outside
    const handleClick = (e) => {
      if (showStyleDropdown && !e.target.closest('.style-dropdown-wrap')) {
        showStyleDropdown = false;
      }
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  });

  onDestroy(() => {
    editorState.editor?.destroy();
  });

  // Sync content when chapter changes (switching tabs)
  $effect(() => {
    if (!editorRaw || !chapter) return;
    // Read chapter to track dependency
    const ch = chapter;
    const currentContent = editorRaw.getHTML();
    const newContent = toHtml(ch.content);
    if (newContent !== currentContent) {
      updatingFromProp = true;
      editorRaw.commands.setContent(newContent, false);
      updatingFromProp = false;
      setTimeout(doScan, 100);
    }
  });

  // Keep a non-reactive ref to the raw editor for decoration use
  let editorRaw = null;

  // Rebuild decorations whenever matches, posMap, or viewedEntityIds change
  $effect(() => {
    // Read reactive dependencies
    const matches = scanMatches;
    const posMap = scanPosMap;
    const viewed = viewedEntityIds;
    // Use non-reactive ref to avoid loop
    if (!editorRaw) return;
    applyingDecorations = true;
    applyDecorations(editorRaw, matches, posMap, viewed);
    applyingDecorations = false;
  });

  function handleMiddleClick(e, id) {
    if (e.button === 1) { e.preventDefault(); onclosetab(id); }
  }

  // Style dropdown helpers
  function getCurrentStyle() {
    const ed = editorState.editor;
    if (!ed) return 'Body';
    if (ed.isActive('heading', { level: 1 })) return 'Title';
    if (ed.isActive('heading', { level: 2 })) return 'Chapter Heading';
    if (ed.isActive('heading', { level: 3 })) return 'Scene Heading';
    return 'Body';
  }

  function setStyle(style) {
    const ed = editorState.editor;
    if (!ed) return;
    if (style === 'body') ed.chain().focus().setParagraph().run();
    else if (style === 'h1') ed.chain().focus().toggleHeading({ level: 1 }).run();
    else if (style === 'h2') ed.chain().focus().toggleHeading({ level: 2 }).run();
    else if (style === 'h3') ed.chain().focus().toggleHeading({ level: 3 }).run();
    showStyleDropdown = false;
  }
</script>

<div class="editor-shell">
  {#if openTabs.length > 0}
    <div class="tab-bar">
      {#each openTabs as tab (tab.id)}
        <div
          class="tab" class:active={tab.id === activeTabId}
          role="tab" tabindex="0"
          onclick={() => onselecttab(tab.id)}
          onkeydown={e => { if (e.key === 'Enter') onselecttab(tab.id); }}
          onmousedown={e => handleMiddleClick(e, tab.id)}
        >
          <span class="tab-label">{tab.title}</span>
          <button class="tab-close" aria-label="Close" onclick={e => { e.stopPropagation(); onclosetab(tab.id); }}>&times;</button>
        </div>
      {/each}
    </div>
  {/if}

  {#if editorState.editor}
    {@const ed = editorState.editor}
    <div class="toolbar">
      <div class="tb-group">
        <button class="tb-btn" onclick={() => ed.chain().focus().undo().run()} title="Undo (Ctrl+Z)" disabled={!ed.can().undo()}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 10h10a5 5 0 015 5v0a5 5 0 01-5 5H11" stroke-linecap="round"/><path d="M7 6L3 10l4 4" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
        <button class="tb-btn" onclick={() => ed.chain().focus().redo().run()} title="Redo (Ctrl+Y)" disabled={!ed.can().redo()}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 10H11a5 5 0 00-5 5v0a5 5 0 005 5h2" stroke-linecap="round"/><path d="M17 6l4 4-4 4" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
      </div>

      <div class="tb-divider"></div>

      <div class="tb-group style-dropdown-wrap">
        <button class="tb-dropdown" onclick={() => showStyleDropdown = !showStyleDropdown}>
          <span class="tb-dropdown-label">{getCurrentStyle()}</span>
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor"><path d="M4 6l4 4 4-4"/></svg>
        </button>
        {#if showStyleDropdown}
          <div class="style-dropdown">
            <button class="style-option" class:active={ed.isActive('paragraph')} onclick={() => setStyle('body')}>
              <span class="style-preview body-preview">Body</span>
              <span class="style-shortcut">Ctrl+Alt+0</span>
            </button>
            <button class="style-option" class:active={ed.isActive('heading', { level: 1 })} onclick={() => setStyle('h1')}>
              <span class="style-preview h1-preview">Title</span>
              <span class="style-shortcut">Ctrl+Alt+1</span>
            </button>
            <button class="style-option" class:active={ed.isActive('heading', { level: 2 })} onclick={() => setStyle('h2')}>
              <span class="style-preview h2-preview">Chapter Heading</span>
              <span class="style-shortcut">Ctrl+Alt+2</span>
            </button>
            <button class="style-option" class:active={ed.isActive('heading', { level: 3 })} onclick={() => setStyle('h3')}>
              <span class="style-preview h3-preview">Scene Heading</span>
              <span class="style-shortcut">Ctrl+Alt+3</span>
            </button>
          </div>
        {/if}
      </div>

      <div class="tb-divider"></div>

      <div class="tb-group">
        <button class="tb-btn" class:on={ed.isActive('bold')} onclick={() => ed.chain().focus().toggleBold().run()} title="Bold (Ctrl+B)">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M6 4h7a4 4 0 014 4v0a4 4 0 01-4 4H6V4z"/><path d="M6 12h8a4 4 0 014 4v0a4 4 0 01-4 4H6v-8z"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('italic')} onclick={() => ed.chain().focus().toggleItalic().run()} title="Italic (Ctrl+I)">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="10" y1="4" x2="14" y2="4"/><line x1="10" y1="20" x2="14" y2="20"/><line x1="14" y1="4" x2="10" y2="20"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('underline')} onclick={() => ed.chain().focus().toggleUnderline().run()} title="Underline (Ctrl+U)">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 4v8a5 5 0 0010 0V4" stroke-linecap="round"/><line x1="5" y1="20" x2="19" y2="20"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('strike')} onclick={() => ed.chain().focus().toggleStrike().run()} title="Strikethrough">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="4" y1="12" x2="20" y2="12"/><path d="M16 7a4 4 0 00-4-3H9a4 4 0 000 8h6a4 4 0 010 8h-4a4 4 0 01-4-3"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('superscript')} onclick={() => ed.chain().focus().toggleSuperscript().run()} title="Superscript">
          <span class="tb-text-icon">A<sup>1</sup></span>
        </button>
        <button class="tb-btn" class:on={ed.isActive('subscript')} onclick={() => ed.chain().focus().toggleSubscript().run()} title="Subscript">
          <span class="tb-text-icon">A<sub>1</sub></span>
        </button>
      </div>

      <div class="tb-divider"></div>

      <div class="tb-group">
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'left' })} onclick={() => ed.chain().focus().setTextAlign('left').run()} title="Align left">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="10" x2="15" y2="10"/><line x1="3" y1="14" x2="18" y2="14"/><line x1="3" y1="18" x2="13" y2="18"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'center' })} onclick={() => ed.chain().focus().setTextAlign('center').run()} title="Center">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="3" y1="6" x2="21" y2="6"/><line x1="6" y1="10" x2="18" y2="10"/><line x1="4" y1="14" x2="20" y2="14"/><line x1="7" y1="18" x2="17" y2="18"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'right' })} onclick={() => ed.chain().focus().setTextAlign('right').run()} title="Align right">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="3" y1="6" x2="21" y2="6"/><line x1="9" y1="10" x2="21" y2="10"/><line x1="6" y1="14" x2="21" y2="14"/><line x1="11" y1="18" x2="21" y2="18"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'justify' })} onclick={() => ed.chain().focus().setTextAlign('justify').run()} title="Justify">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/><line x1="3" y1="14" x2="21" y2="14"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
        </button>
      </div>

      <div class="tb-divider"></div>

      <div class="tb-group">
        <button class="tb-btn" class:on={ed.isActive('bulletList')} onclick={() => ed.chain().focus().toggleBulletList().run()} title="Bullet list">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="9" y1="6" x2="20" y2="6"/><line x1="9" y1="12" x2="20" y2="12"/><line x1="9" y1="18" x2="20" y2="18"/><circle cx="5" cy="6" r="1" fill="currentColor" stroke="none"/><circle cx="5" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="5" cy="18" r="1" fill="currentColor" stroke="none"/></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('orderedList')} onclick={() => ed.chain().focus().toggleOrderedList().run()} title="Numbered list">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="10" y1="6" x2="20" y2="6"/><line x1="10" y1="12" x2="20" y2="12"/><line x1="10" y1="18" x2="20" y2="18"/><text x="3" y="8" font-size="7" fill="currentColor" stroke="none" font-family="serif">1</text><text x="3" y="14" font-size="7" fill="currentColor" stroke="none" font-family="serif">2</text><text x="3" y="20" font-size="7" fill="currentColor" stroke="none" font-family="serif">3</text></svg>
        </button>
        <button class="tb-btn" class:on={ed.isActive('blockquote')} onclick={() => ed.chain().focus().toggleBlockquote().run()} title="Block quote">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M10 7v4H6v6h4v-6l-1-4h1zm8 0v4h-4v6h4v-6l-1-4h1z" opacity="0.7"/></svg>
        </button>
        <button class="tb-btn" onclick={() => ed.chain().focus().setHorizontalRule().run()} title="Scene break ( * * * )">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="6" cy="12" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="18" cy="12" r="1.5"/></svg>
        </button>
      </div>
    </div>
  {/if}

  <div class="editor-scroll" oncontextmenu={handleContextMenu}
    onclick={() => { closeCtxMenu(); }}>
    <div bind:this={element} class="editor-page"></div>
    {#if !chapter}
      <div class="editor-empty">
        <p>Select or create a chapter to begin.</p>
      </div>
    {/if}

    {#each suggestions as s (s.id)}
      <div class="suggestion-bubble" style="top: {s.top}px; left: {s.left}px;">
        <div class="suggestion-progress" style="animation-duration: {SUGGESTION_DURATION}ms;"></div>
        <div class="suggestion-header">
          <span class="suggestion-word">{s.word}</span>
          <button class="suggestion-dismiss" onclick={() => dismissSuggestion(s.id)}>&times;</button>
        </div>
        <div class="suggestion-actions">
          <button class="suggestion-btn add" onclick={() => handleSuggestionAdd(s)}>+ Add Entity</button>
          <button class="suggestion-btn ignore" onclick={() => handleSuggestionIgnore(s)}>Ignore</button>
        </div>
      </div>
    {/each}
  </div>

  {#if ctxMenu.show}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ctx-backdrop" onclick={closeCtxMenu} oncontextmenu={(e) => { e.preventDefault(); closeCtxMenu(); }}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="ctx-menu" style="left: {ctxMenu.x}px; top: {ctxMenu.y}px;" onclick={(e) => e.stopPropagation()}>
        {#if ctxMenu.isEntity}
          <button class="ctx-item" onclick={ctxGoToDefinition}>
            <i class="bi bi-link-45deg ctx-icon"></i>
            Go to definition
          </button>
          <button class="ctx-item" onclick={ctxFindReferences}>
            <i class="bi bi-search ctx-icon"></i>
            Find references
          </button>
        {/if}
        {#if ctxMenu.isMisspelled}
          <button class="ctx-item" onclick={ctxOpenModal}>
            <i class="bi bi-spellcheck ctx-icon"></i>
            Spelling & Synonyms...
          </button>
          <button class="ctx-item" onclick={ctxAddToDictionary}>
            <i class="bi bi-plus-circle ctx-icon"></i>
            Add to dictionary
          </button>
        {:else}
          <button class="ctx-item" onclick={ctxOpenModal}>
            <i class="bi bi-journal-text ctx-icon"></i>
            Synonyms...
          </button>
        {/if}
        {#if ctxMenu.isEntity || ctxMenu.isMisspelled}
          <div class="ctx-divider"></div>
        {/if}
        {#if !ctxMenu.isEntity}
          <button class="ctx-item" onclick={ctxCreateEntity}>
            <i class="bi bi-person-plus ctx-icon"></i>
            Create entity "{ctxMenu.word}"
          </button>
        {/if}
        <button class="ctx-item" onclick={ctxIgnore}>
          <i class="bi bi-eye-slash ctx-icon"></i>
            Ignore "{ctxMenu.word}"
          </button>
      </div>
    </div>
  {/if}

  <WordModal
    show={wordModal.show}
    word={wordModal.word}
    isMisspelled={wordModal.isMisspelled}
    suggestions={wordModal.suggestions}
    editorFrom={wordModal.from}
    editorTo={wordModal.to}
    onreplace={handleModalReplace}
    ondictionaryadd={handleModalDictionaryAdd}
    onclose={handleModalClose}
  />
</div>

<style>
  .editor-shell {
    display: flex; flex-direction: column; height: 100%;
    background: var(--iwe-bg-warm);
  }

  .tab-bar {
    display: flex; flex-shrink: 0; overflow-x: auto;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-sidebar); padding: 0 0.5rem;
  }
  .tab {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.75rem; cursor: pointer;
    font-size: 0.8rem; color: var(--iwe-text-muted);
    border-bottom: 2px solid transparent;
    white-space: nowrap; flex-shrink: 0; transition: all 150ms;
  }
  .tab:hover { color: var(--iwe-text-secondary); background: var(--iwe-bg-hover); }
  .tab.active { color: var(--iwe-text); border-bottom-color: var(--iwe-accent); background: var(--iwe-bg-warm); }
  .tab-close {
    background: none; border: none; font-size: 1rem;
    color: var(--iwe-text-faint); cursor: pointer; line-height: 1;
    padding: 0 2px; border-radius: 2px;
  }
  .tab-close:hover { color: var(--iwe-danger); }

  .toolbar {
    display: flex; align-items: center; gap: 0.15rem;
    padding: 0.4rem 0.6rem; flex-shrink: 0;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg); flex-wrap: wrap;
  }
  .tb-group { display: flex; align-items: center; gap: 1px; }
  .tb-divider {
    width: 1px; height: 22px; background: var(--iwe-border-light);
    margin: 0 0.4rem; flex-shrink: 0;
  }
  .tb-btn {
    background: none; border: 1px solid transparent;
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text-secondary); padding: 0.3rem 0.4rem;
    display: inline-flex; align-items: center; justify-content: center;
    transition: all 100ms; min-width: 28px; height: 28px;
  }
  .tb-btn:hover {
    background: var(--iwe-bg-hover); color: var(--iwe-text);
    border-color: var(--iwe-border);
  }
  .tb-btn.on {
    background: var(--iwe-accent-light); color: var(--iwe-accent);
    border-color: transparent;
  }
  .tb-btn:disabled { opacity: 0.3; cursor: default; }
  .tb-btn:disabled:hover { background: none; border-color: transparent; color: var(--iwe-text-secondary); }
  .tb-text-icon { font-family: var(--iwe-font-ui); font-size: 0.75rem; font-weight: 500; line-height: 1; }
  .tb-text-icon sup, .tb-text-icon sub { font-size: 0.55rem; }

  .style-dropdown-wrap { position: relative; }
  .tb-dropdown {
    background: none; border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm); cursor: pointer;
    color: var(--iwe-text); padding: 0.25rem 0.5rem;
    display: inline-flex; align-items: center; gap: 0.4rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    height: 28px; min-width: 130px; transition: all 100ms;
  }
  .tb-dropdown:hover { border-color: var(--iwe-accent); }
  .tb-dropdown-label { flex: 1; text-align: left; }
  .style-dropdown {
    position: absolute; top: 100%; left: 0; z-index: 100;
    margin-top: 4px; min-width: 220px;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); padding: 0.35rem;
    box-shadow: 0 6px 20px rgba(0,0,0,0.08), 0 1px 4px rgba(0,0,0,0.04);
  }
  .style-option {
    display: flex; align-items: center; justify-content: space-between;
    width: 100%; background: none; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.45rem 0.6rem; text-align: left; transition: background 100ms;
  }
  .style-option:hover { background: var(--iwe-bg-hover); }
  .style-option.active { background: var(--iwe-accent-light); }
  .style-preview { color: var(--iwe-text); }
  .body-preview { font-family: var(--iwe-font-prose); font-size: 0.9rem; }
  .h1-preview { font-family: var(--iwe-font-prose); font-size: 1.1rem; font-weight: 700; }
  .h2-preview { font-family: var(--iwe-font-prose); font-size: 1rem; font-weight: 700; }
  .h3-preview { font-family: var(--iwe-font-prose); font-size: 0.9rem; font-weight: 700; font-style: italic; }
  .style-shortcut { font-size: 0.65rem; color: var(--iwe-text-faint); font-family: var(--iwe-font-ui); }

  .editor-scroll {
    flex: 1; overflow-y: auto; position: relative;
    background: var(--iwe-paper);
  }
  .editor-page {
    max-width: 720px; margin: 0 auto;
    padding: 2.5rem 3rem 6rem; min-height: 100%;
  }
  .editor-page :global(.prose-editor) {
    outline: none; font-family: var(--iwe-font-prose);
    font-size: 1.05rem; line-height: 1.9;
    color: var(--iwe-text); min-height: 400px;
  }
  .editor-page :global(.prose-editor p) { margin: 0 0 0.8em; }
  .editor-page :global(.prose-editor h1) {
    font-family: var(--iwe-font-prose); font-size: 1.75rem; font-weight: 700;
    margin: 2em 0 0.6em; line-height: 1.3;
  }
  .editor-page :global(.prose-editor h2) {
    font-family: var(--iwe-font-prose); font-size: 1.35rem; font-weight: 700;
    margin: 1.5em 0 0.5em; line-height: 1.3;
  }
  .editor-page :global(.prose-editor h3) {
    font-family: var(--iwe-font-prose); font-size: 1.1rem; font-weight: 700;
    margin: 1.25em 0 0.4em; line-height: 1.4; font-style: italic;
  }
  .editor-page :global(.prose-editor blockquote) {
    border-left: 2px solid var(--iwe-text-faint);
    padding-left: 1.25rem; margin: 1.25em 0;
    color: var(--iwe-text-secondary); font-style: italic;
  }
  .editor-page :global(.prose-editor hr) { border: none; text-align: center; margin: 2.5em 0; }
  .editor-page :global(.prose-editor hr::after) {
    content: '* * *'; font-family: var(--iwe-font-prose);
    color: var(--iwe-text-faint); letter-spacing: 0.5em; font-size: 0.85rem;
  }
  .editor-page :global(.prose-editor ul),
  .editor-page :global(.prose-editor ol) { padding-left: 1.75rem; margin: 0.5em 0; }
  .editor-page :global(.prose-editor li) { margin-bottom: 0.25em; }
  .editor-page :global(.tiptap p.is-editor-empty:first-child::before) {
    color: var(--iwe-text-faint); content: attr(data-placeholder);
    float: left; height: 0; pointer-events: none; font-style: italic;
  }
  .editor-empty {
    position: absolute; inset: 0;
    display: flex; align-items: center; justify-content: center;
    color: var(--iwe-text-faint);
    font-family: var(--iwe-font-prose); font-style: italic;
  }

  /* Live suggestion bubbles */
  .suggestion-bubble {
    position: absolute; z-index: 50;
    display: flex; flex-direction: column; gap: 0.25rem;
    padding: 0.4rem 0.5rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); box-shadow: 0 4px 16px rgba(0,0,0,0.1);
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    animation: bubbleFade 0.2s ease;
    width: 100px; overflow: hidden;
  }
  @keyframes bubbleFade {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .suggestion-progress {
    position: absolute; bottom: 0; left: 0; height: 2px;
    background: var(--iwe-accent);
    border-radius: 0 0 var(--iwe-radius) 0;
    pointer-events: none;
    width: 100%;
    animation: progressShrink linear forwards;
  }
  @keyframes progressShrink {
    from { width: 100%; }
    to { width: 0%; }
  }
  .suggestion-header {
    display: flex; align-items: center; justify-content: space-between; gap: 0.5rem;
  }
  .suggestion-word {
    font-weight: 600; color: var(--iwe-text); font-size: 0.8rem;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .suggestion-actions {
    display: flex; flex-direction: column; gap: 0.15rem;
  }
  .suggestion-btn {
    font-family: var(--iwe-font-ui); font-size: 0.65rem; font-weight: 500;
    padding: 0.2rem 0.3rem; border-radius: var(--iwe-radius-sm);
    cursor: pointer; border: 1px solid var(--iwe-border); transition: all 100ms;
    text-align: center; width: 100%;
  }
  .suggestion-btn.add {
    background: var(--iwe-accent-light); color: var(--iwe-accent); border-color: var(--iwe-accent);
  }
  .suggestion-btn.add:hover { background: var(--iwe-accent); color: white; }
  .suggestion-btn.ignore {
    background: none; color: var(--iwe-text-faint);
  }
  .suggestion-btn.ignore:hover { border-color: var(--iwe-danger); color: var(--iwe-danger); }
  .suggestion-dismiss {
    background: none; border: none; color: var(--iwe-text-faint);
    cursor: pointer; font-size: 1rem; line-height: 1; padding: 0 2px;
  }
  .suggestion-dismiss:hover { color: var(--iwe-text); }

  /* Spell error underlines */
  .editor-page :global(.spell-error) {
    text-decoration: wavy underline;
    text-decoration-color: var(--iwe-danger, #b85450);
    text-underline-offset: 3px;
    text-decoration-thickness: 1.5px;
  }

  /* Context menu */
  .ctx-backdrop {
    position: fixed; inset: 0; z-index: 1000;
  }
  .ctx-menu {
    position: fixed; z-index: 1001;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); padding: 0.3rem;
    box-shadow: 0 8px 24px rgba(0,0,0,0.12), 0 2px 6px rgba(0,0,0,0.06);
    min-width: 200px;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    animation: ctxFade 0.1s ease;
  }
  @keyframes ctxFade {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }
  .ctx-item {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; background: none; border: none; border-radius: var(--iwe-radius-sm);
    cursor: pointer; padding: 0.45rem 0.6rem; text-align: left;
    color: var(--iwe-text); transition: background 80ms;
  }
  .ctx-item:hover { background: var(--iwe-bg-hover); }
  .ctx-icon { color: var(--iwe-text-muted); flex-shrink: 0; font-size: 0.85rem; width: 1rem; text-align: center; }
  .ctx-divider {
    height: 1px; background: var(--iwe-border-light);
    margin: 0.2rem 0.4rem;
  }
</style>
