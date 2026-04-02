# IWE — Integrated Writing Environment

An IDE for novelists. Characters, places, and things are the symbols. The manuscript is the codebase.

## Quick Start

```bash
npm install           # Install frontend dependencies
npm run tauri dev     # Run the app in development mode
```

Rust builds automatically via Tauri. The Cargo.toml is in `src-tauri/`.

## Tech Stack

- **Frontend:** Svelte 5 (runes mode), SvelteKit with `adapter-static`, vanilla JS (no TypeScript)
- **Editor:** TipTap (ProseMirror-based) with custom extensions
- **Document Model:** Yjs (`yrs` in Rust, `yjs` + `y-prosemirror` in JS) — every character has a permanent identity
- **Backend:** Tauri v2 (Rust), rusqlite with bundled SQLite
- **Scanner:** Aho-Corasick (Rust) for blazing-fast multi-pattern entity matching
- **PDF Export:** genpdf with embedded Liberation Serif fonts
- **DOCX Export:** docx npm package
- **Styling:** Bootstrap 5 + custom CSS theme (`src/lib/theme.css`), Bootstrap Icons
- **Drag & Drop:** svelte-dnd-action

## Architecture

### Yjs Document Model (critical)

Chapter content is stored as **Yjs binary state** (BLOB), not HTML strings. Both Rust (`yrs` crate) and JS (`yjs` + `y-prosemirror`) share the same document model.

**Why:** Both sides read from the same Y.Doc, so plain text extraction is guaranteed identical. No more fragile HTML stripping ↔ `buildTextMap()` coordinate drift. Every character has a permanent identity that survives edits, enabling stable position references for features like comments.

**How it works:**

1. **Storage:** `chapters.content` is a `BLOB` column in SQLite containing Y.Doc state encoded via `yrs::Update`
2. **Editor binding:** TipTap binds to a `Y.XmlFragment` via `ySyncPlugin` from `y-prosemirror`. The fragment name is `'prosemirror'` (y-prosemirror convention)
3. **Saving:** On content change, `Y.encodeStateAsUpdate(doc)` produces a `Uint8Array` sent to Rust for storage
4. **Loading:** Rust returns the BLOB as `Vec<u8>` (serialized as `number[]` over Tauri IPC), JS applies it to a fresh `Y.Doc` via `Y.applyUpdate()`
5. **Chapter switching:** The entire TipTap `Editor` instance is destroyed and recreated — `ySyncPlugin` binds to a specific `XmlFragment` at creation time and cannot be rebound

**Key files:**
- `src-tauri/src/ydoc.rs` — `load_doc()`, `encode_doc()`, `extract_plain_text()`, `extract_text_with_breaks()`, `word_count()`
- `src/lib/ydoc.js` — `createChapterDoc()`, `encodeDoc()`, `destroyDoc()`

**Undo/Redo:** StarterKit's `history` plugin is disabled (`history: false`). Undo/redo uses `yUndoPlugin` from `y-prosemirror` with `undo`/`redo` commands bound to `Mod-z`/`Mod-y`/`Mod-Shift-z`.

**Text extraction:** `ydoc::extract_plain_text()` walks the Y.Doc's `XmlFragment` depth-first, concatenating text from `XmlText` nodes with NO separators between blocks. `extract_text_with_breaks()` inserts `\n\n` between top-level blocks (used for PDF export and dialogue detection with paragraph boundaries).

**IMPORTANT — yrs vs PM text drift:** `extract_plain_text()` (Rust/yrs) and `buildTextMap()` (JS/PM) can produce slightly different text when the document contains formatting marks (bold, italic) or atom nodes (noteMarker, stateMarker). `buildTextMap` is the authoritative source for position mapping. Rust char offsets should only be used as proximity hints for disambiguation, never as direct indices into `posMap`. See Navigation section.

**Word counts:** Since chapter content is binary, word counting uses a Rust command `get_all_chapter_word_counts` that loads each chapter's Y.Doc and counts words. The frontend caches these in a `wordCounts` state object.

**Exports:** For DOCX/TXT/HTML export, chapters need HTML. The page uses `yDocToProsemirrorJSON()` from `y-prosemirror` + `generateHTML()` from `@tiptap/core` to convert Y.Doc state to HTML on demand. PDF export uses `extract_text_with_breaks()` in Rust.

### All database access goes through Rust

There is NO `tauri-plugin-sql`. All SQLite operations are Rust commands invoked via `@tauri-apps/api/core`. The frontend JS never touches SQL directly.

**Data flow:** Svelte component → `src/lib/db.js` (thin `invoke()` wrappers) → Tauri command in `src-tauri/src/lib.rs` → database function in `src-tauri/src/db.rs` → rusqlite

### Entity scanning is Rust-side

The Aho-Corasick scanner lives in `src-tauri/src/scanner.rs`. It reads entities from the DB, builds search patterns, and scans text in a single pass. The JS side sends text and receives match results. Never implement scanning in JS.

The scanner extracts plain text from Y.Doc via `ydoc::extract_plain_text()` for all multi-chapter commands (`scan_all_chapters`, `find_references`, `text_search`, etc.). The `scan_text` command receives pre-extracted plain text directly from the frontend (via `buildTextMap`).

### Editor reactivity pattern (critical)

TipTap lives outside Svelte's reactivity. The official Svelte 5 pattern is used:

```js
let editorState = $state({ editor: null });
// In onTransaction:
editorState = { editor }; // reassign the OBJECT to trigger reactivity
```

Entity highlight decorations are managed via a ProseMirror plugin (`entityHighlight.js`). Scan results and viewed entity IDs live in Svelte `$state`. A `$effect` watches them and calls `applyDecorations()` which dispatches a PM transaction. An `applyingDecorations` flag prevents infinite loops (decoration transaction → onTransaction → editorState reassign → $effect).

### Comments / Notes System

Users annotate their manuscript by highlighting text and right-clicking → "Add a note". Comments are **highlight-based** (like Google Docs) — the commented text range gets a colored background. No icons or gutter markers.

**Architecture:**
- A `noteMarker` custom TipTap node (inline, atom, zero-width invisible) anchors each comment's position in the document. It stores `commentId` (DB row ID) and `highlightLen` (chars after the marker to highlight)
- The note marker lives in the Y.Doc and moves naturally with text edits
- A ProseMirror plugin (`commentDecoKey`) renders inline decorations over the highlighted text ranges
- The `comments` table in SQLite stores only `chapter_id` and `note_text` — position is implicit in the document
- Clicking highlighted text dispatches a `comment-highlight-click` custom event → opens the Notes panel

**Visual states:**
- Unselected comments: very faint yellow highlight (12% opacity)
- Active/selected comment: strong yellow highlight (35% opacity, 2px border)
- Hover on any comment: brightens slightly

**Notes panel** (right panel tab): List → detail pattern. Clicking a note navigates to it and shows the detail view with editable textarea. "Update highlight" button (only visible when text is selected in editor) moves the note to the current selection. Two-step inline delete confirmation.

**Key files:**
- `src/lib/noteMarker.js` — `NoteMarker` node extension, `commentDecoKey` plugin, `applyCommentHighlights()`
- `src/lib/components/NotesPanel.svelte` — List and detail views
- `src-tauri/src/db.rs` — `comments` table, `Comment` struct, CRUD

### Entity State Tracking (checkpoint model)

Writers track how entities change throughout the manuscript by placing **state markers** (visible ◆ diamond icons) in the document and attaching key-value facts and entity references to them.

**Checkpoint model:** A state marker is a "checkpoint" — a Y.Doc inline atom node that can hold multiple key-value pairs and entity references. Each checkpoint says "at this point in the story, these things are true about this entity." State is resolved by accumulating all checkpoints in **story-time order** up to the cursor position — later checkpoints overwrite earlier values for the same key.

**Two value types per checkpoint:**
- **Key/Value facts** — freeform text pairs (e.g. `weight = 100kg`, `mood = angry`). Once a key name is saved, it cannot be renamed — only values change at subsequent checkpoints.
- **Entity references** — links to other entities with an Active/Inactive toggle (e.g. `The Sword → Active`). Used for tracking relationships/possessions.

**Data model (two tables):**
- `state_markers` — parent row per marker (entity_id, chapter_id, note, timestamps)
- `state_marker_values` — child rows per checkpoint (value_type='fact'|'entity_ref', fact_key, fact_value, ref_entity_id, ref_active)

**State tab in EntityPanel** has three modes:
1. **Resolved view** (default) — shows accumulated state at cursor position, dynamically recalculated as cursor moves (300ms debounce). Two sub-views: "State at cursor" and "All state changes" (chronological list of every checkpoint). Also shows "Referenced by" section for incoming entity refs.
2. **Checkpoint editor** — opened by clicking a ◆ marker in the editor. Shows all entity variables (both this checkpoint's values and inherited values from earlier checkpoints). Save button persists changes, Cancel discards. Two add buttons: "Add variable" and "Add entity reference".
3. **Filters** — name and value text filters on the resolved view for trimming large variable lists.

**State resolution uses story-time order, not document order.** The `resolveUpTo()` function in EntityPanel loads `time_section_order` from the DB, determines which time section each marker is in via `getTimeSectionForPos()`, and uses the section's story_order to position markers chronologically. A flashback section ordered before Chapter 1 in the Time Flow Manager will have its state checkpoints resolved before Chapter 1's.

**Key files:**
- `src/lib/stateMarker.js` — `StateMarker` TipTap node (inline, atom, selectable, draggable), click handler plugin
- `src/lib/components/EntityPanel.svelte` — State sub-tab, `resolveUpTo()`, `buildEditRows()`, `resolveStateAtCursor()`
- `src-tauri/src/db.rs` — `state_markers` + `state_marker_values` tables, `StateMarker`/`StateMarkerValue` structs, CRUD

**Creating state markers:** Right-click an entity mention → "Set state value" → inserts ◆ marker at cursor → opens entity panel in checkpoint editor mode. The editor pre-shows all existing entity variables with their resolved values at that position.

**Clicking state markers:** Clicking a ◆ in the editor fires `state-marker-click` event → page looks up the marker's entity via `getStateMarker(id)` → opens entity panel to that entity's state tab in checkpoint editor mode.

### Time Flow System (non-linear narratives)

Handles flashbacks, flash-forwards, and time jumps. Writers wrap text in **time break regions** — a named start divider + content + end divider. The Time Flow Manager reorders these regions in story-time order.

**Time break node (`src/lib/timeBreak.js`):** A wrapping TipTap block node (like blockquote). Contains `block+` content. Has a `label` attribute (editable, e.g. "Fifteen years earlier"). Rendered with a start divider (editable label), content area (teal left border), and end divider ("End time jump"). Nesting is prevented — `insertTimeBreak` checks parent nodes.

**Time Flow Manager (`src/routes/timeflow/+page.svelte`):** Popup window showing all time sections as draggable cards. Chapters without time breaks = one card. Chapters with time breaks = multiple cards (flow sections + time-jumped sections). Drag to reorder in story-time order. Saves to `time_section_order` table.

**Data model:**
- `time_section_order` — chapter_id, section_index, label, story_order (global sort key). When empty, system falls back to chapter sort_order (zero setup required).

**How sections are extracted (Rust):** `ydoc::extract_time_sections()` walks the Y.Doc top-level children. Regular blocks accumulate into flow sections. `timeBreak` wrapper nodes become their own sections with the label attribute. Returns `Vec<TimeSection>` with section_index, label, is_flow, preview_text, word_count.

**How entity state uses time flow:** `resolveUpTo()` in EntityPanel builds a story order map from `time_section_order`, determines each marker's time section via `getTimeSectionForPos()`, and uses story_order for ordering. Within the same section, document position is the tiebreaker.

**Key files:**
- `src/lib/timeBreak.js` — `TimeBreak` wrapping TipTap node
- `src/routes/timeflow/+page.js` + `+page.svelte` — Time Flow Manager popup
- `src-tauri/src/ydoc.rs` — `extract_time_sections()`, `locate_state_markers_in_sections()`
- `src-tauri/capabilities/default.json` — includes `"timeflow*"`, `"pacing*"`

### Navigation / Jump-to-Position (CRITICAL — read this carefully)

`handleGoToChapter(chapterId, searchText, positionHint)` in the project page is the **ONE central function** for ALL click-to-navigate-and-highlight across the entire app. Every feature that lets the user click a result and jump to a position in the editor MUST use this function. Do not create alternative navigation paths.

**Used by:** find-all-references, text search, dialogue search, relationship search, similar phrasing, word frequency browser, cluster finder, entity detection go-to, pinned excerpt clicks, pacing analysis, adverb analysis.

**CRITICAL — `buildTextMap` is the single source of truth:**

`buildTextMap(doc)` walks the ProseMirror doc and builds `text` (plain string) and `posMap` (array where `posMap[charIndex]` = PM doc position). ALL position resolution goes through this. Rust `char_position` values from `extract_plain_text()` (yrs) can drift from `buildTextMap` positions due to formatting marks (bold/italic) and atom nodes (noteMarker, stateMarker). **Never use Rust char positions as direct indices into `posMap`.**

**How it works:**

1. **Primary — text search with hint:** `searchText` is found in `buildTextMap`'s text using `findTextNearHint()`, which locates all occurrences and picks the one nearest to the Rust `positionHint`. The hint is only for disambiguation when `searchText` appears multiple times — it is never used as a direct position.

2. **Entity notes — pre-resolved PM positions:** When `positionHint` is `{pmFrom, pmTo}`, these are Y.Doc relative positions already resolved to PM positions via `y-prosemirror`. Used only for entity pinned excerpts.

**When adding new features that navigate:**
- Pass `searchText` (the text to highlight) and a Rust `char_position` as a proximity hint (number)
- The hint does NOT need to be exact — it just disambiguates which occurrence to jump to
- ALWAYS call `handleGoToChapter()` — never implement your own text search or position mapping
- NEVER index directly into `posMap` with a Rust char offset — always use `findTextNearHint()`
- The function also handles: opening the chapter tab, scroll-to-center, and the yellow flash highlight

**Cross-window navigation:** Popup windows (pacing, etc.) emit `navigate-to-position` events via `emitTo('main', ...)` from `@tauri-apps/api/event`. The main window listens and calls `handleGoToChapter`.

### Multi-window architecture

Pop-up windows (heatmap, chapter analysis, stats, time flow manager, pacing analysis) are separate SvelteKit routes opened via `WebviewWindow` from `@tauri-apps/api/webviewWindow`. They read data directly from the database — no data passing between windows. Each window needs its route in `src/routes/` and its label pattern in `src-tauri/capabilities/default.json`.

**Cross-window events:** Popup windows can navigate the main editor via `emitTo('main', 'navigate-to-position', payload)` from `@tauri-apps/api/event`. The main window listens for this event and calls `handleGoToChapter`. Payload: `{ chapterId, searchText, charPosition }`.

The print preview window converts Y.Doc bytes to HTML via `yDocToProsemirrorJSON()` + `generateHTML()` before rendering.

## Project Structure

```
src/
├── routes/
│   ├── +page.svelte              # Home page (project list)
│   ├── project/[filename]/       # Main editor workspace
│   ├── heatmap/                  # Entity heatmap (popup window)
│   ├── chapters/                 # Chapter analysis (popup window)
│   ├── stats/                    # Writing stats (popup window)
│   ├── timeflow/                 # Time Flow Manager (popup window)
│   ├── pacing/                   # Pacing analysis — sentence length waveforms (popup window)
│   └── print/                    # Print preview (popup window)
├── lib/
│   ├── db.js                     # ALL Tauri command wrappers
│   ├── entityHighlight.js        # ProseMirror decoration plugins (entities + spellcheck + debug)
│   ├── noteMarker.js             # NoteMarker TipTap node + comment highlight decorations
│   ├── stateMarker.js            # StateMarker TipTap node (entity state checkpoints)
│   ├── timeBreak.js              # TimeBreak TipTap wrapping node (time jump regions)
│   ├── ydoc.js                   # Y.Doc lifecycle (create, encode, destroy)
│   ├── export.js                 # DOCX/HTML/TXT/PDF export
│   ├── theme.css                 # Design system CSS variables
│   ├── toast.js                  # Toast notification store
│   └── components/
│       ├── Editor.svelte         # TipTap editor + toolbar + suggestions + spellcheck + context menu + comments + state markers + time breaks
│       ├── WordModal.svelte      # Combined spelling & synonym modal
│       ├── EntityPanel.svelte    # Entity CRUD + notes + detection + state tracking
│       ├── NotesPanel.svelte     # Comments/notes list + detail view
│       ├── ChapterNav.svelte     # Chapter sidebar
│       ├── SearchPanel.svelte    # Text/dialogue/relationship search
│       ├── AnalysisPanel.svelte  # Analysis tools (frequency/clusters/similar/adverbs/dialogue detection/heatmap/pacing/chapters)
│       └── Toasts.svelte         # Toast renderer

src-tauri/
├── src/
│   ├── main.rs                   # Entry point
│   ├── lib.rs                    # Tauri commands + PDF export
│   ├── db.rs                     # Database schema + all queries
│   ├── scanner.rs                # Aho-Corasick scanner + entity/search commands
│   ├── analysis.rs               # Analysis tools (frequency, clusters, similar, pacing, adverbs, heatmap, chapter analysis)
│   ├── text_utils.rs             # Shared text utilities (dialogue extraction, sentence extraction, word counting)
│   ├── ydoc.rs                   # Y.Doc load/encode/text extraction (yrs)
│   ├── spellcheck.rs             # Hunspell dictionary + spell checking + custom words
│   ├── synonyms.rs               # Moby Thesaurus lookup (in-memory)
│   └── wordlists.rs              # Verb/adjective/adverb lists for POS tags
├── fonts/
│   └── LiberationSerif-*.ttf     # Embedded fonts for PDF export
├── resources/
│   ├── dictionaries/en_US.*      # Hunspell dictionary (embedded via include_str!)
│   └── mthesaur.txt              # Moby Thesaurus II (embedded via include_str!)
├── capabilities/default.json     # Tauri permissions
└── Cargo.toml
```

## Database Schema (16 tables)

- **chapters** — id, title, content (BLOB — Yjs state), sort_order, timestamps
- **entities** — id, name, entity_type (character/place/thing), description, color, visible
- **aliases** — entity_id, alias text
- **entity_fields** — key-value custom fields (schema exists, UI not built)
- **ignored_words** — words excluded from entity detection
- **custom_words** — spell checker custom dictionary (word, source: 'user' or 'entity')
- **entity_notes** — pinned text ranges from chapters stored as Y.Doc relative positions (y_start BLOB, y_end BLOB), with sort_order. Text is resolved live from the Y.Doc — ranges expand if text is inserted within them.
- **entity_free_notes** — free-form note cards per entity, with sort_order
- **comments** — chapter_id, note_text, timestamps (position lives in the Y.Doc as a noteMarker node)
- **state_markers** — entity_id, chapter_id, note, timestamps (position lives in Y.Doc as a stateMarker node)
- **state_marker_values** — marker_id, value_type ('fact'|'entity_ref'), fact_key, fact_value, ref_entity_id, ref_active
- **time_section_order** — chapter_id, section_index, label, story_order (global sort key for time flow)
- **writing_activity** — per-save log (timestamp, chapter, word counts, delta)
- **daily_stats** — aggregated daily (words added/deleted/net, active minutes)
- **writing_settings** — daily_goal, session_gap_minutes (singleton)
- **nav_history** — chapter_id, scroll_top, cursor_pos (max 100)

Migrations are handled in `init_schema()` with `CREATE TABLE IF NOT EXISTS` for idempotent creation and `ALTER TABLE ... ADD COLUMN` checks for backward compatibility with existing `.iwe` files. During active development, schema changes may require deleting the `.iwe` file and starting fresh.

## Key Design Decisions

### Yjs as document model (not for collaboration)
Yjs is used as a **document model**, not for collaboration. Every character gets a permanent identity. This makes position references stable across edits — critical for comments, entity state tracking, and future features like inline notes and timeline markers. The `yrs` Rust crate provides text extraction; `y-prosemirror` binds TipTap to the Y.Doc.

### Comments are highlight-based, not icon-based
Comments are anchored by an invisible zero-width `noteMarker` node in the document. The visible indicator is a highlight decoration over the text range. This avoids the complexity of positioning gutter icons and is familiar UX (Google Docs pattern). The `noteMarker` node stores `commentId` and `highlightLen`; the actual note text lives in the `comments` DB table.

### Entity names and aliases are treated equally
Both go through `build_terms_filtered()` into Aho-Corasick patterns. Aliases are trimmed on save. `MatchKind::LeftmostLongest` ensures multi-word aliases like "Sloane Maker" aren't shadowed by the shorter entity name "Sloane".

### Possessive handling
The scanner accepts matches followed by `'s` or `'s` (curly apostrophe). The entity detection normalizes possessives before checking the known set.

### Entity visibility (eye toggle)
Visibility is stored in the DB (`visible` column). `build_terms()` only includes visible entities for editor highlighting and chapter counts. `build_terms_all()` ignores visibility for search and find-references. The eye state persists across sessions.

### Hard-excluded words
`HARD_EXCLUDE` in `scanner.rs` contains ~130 common contractions, pronouns, days, months etc. Both straight and curly apostrophe variants are added. Used by both `detect_entities` and `check_word`.

### Live entity suggestion bubbles
Every 4 keystrokes, `checkForSuggestion()` in Editor.svelte checks the last 150 chars for capitalized mid-sentence words. Calls Rust `check_word` to verify against known entities + ignored + hard excludes. Shows floating bubbles with 30-second CSS-animated progress bars. Multiple bubbles can show simultaneously.

### Spell checking
Uses a hand-rolled Hunspell-compatible dictionary parser (`spellcheck.rs`) that reads `en_US.dic` + `en_US.aff` (embedded via `include_str!`), expands affix rules, and builds a `HashSet<String>` of ~180K words at startup. Spell checking runs after entity scanning (400ms debounce) — words covered by entity decorations are skipped. Suggestions use edit-distance-1 and edit-distance-2 candidates. The `custom_words` table stores per-project additions with source tracking ('user' vs 'entity'). Entity names/aliases are auto-synced to the custom dictionary on create/update/delete.

### Synonyms (Moby Thesaurus)
The Moby Thesaurus II (~30K entries) is embedded via `include_str!` and parsed into an in-memory `HashMap` at startup (`synonyms.rs`). Lookups try the exact word first, then strip common suffixes (-s, -ed, -ing, -ly, etc.) to find base forms. Returns up to 200 synonyms per word.

### State markers are Y.Doc nodes, not text offsets
State markers are inline atom nodes in the Y.Doc — they have permanent identity and survive edits. Navigation to a state marker uses `getStateMarkerPositions()` which finds the node by its `stateId` attribute directly in the ProseMirror doc. No word-sequence searching needed. Markers are selectable and draggable — users can cut/paste them to relocate.

### State resolution is story-time-aware
The `resolveUpTo()` function in EntityPanel doesn't use document order. It loads `time_section_order` from the DB, determines each marker's time section via `getTimeSectionForPos()`, and orders markers by their section's `story_order`. This means a state checkpoint inside a flashback that's been positioned before Chapter 1 in the Time Flow Manager will be resolved before Chapter 1's checkpoints. Fallback when no time flow ordering exists: chapter sort_order × 1000 + section_index.

### Never mutate the document for visual effects
Debug overlays, analysis highlights, and any temporary visual decoration MUST use ProseMirror decoration plugins (via PluginKey + setMeta pattern), never TipTap mark/node mutations. Applying bold/italic/etc. as a "highlight" modifies the Y.Doc, gets auto-saved, and corrupts the author's text. Use `debugDecoKey` plugin or create a new PluginKey for new visual overlays.

### Time breaks are wrapping nodes, not dividers
A time break is a **wrapping** block node (like blockquote), not a single divider. It has a start divider (with editable label), content area, and end divider. Content inside the wrapper is the time-jumped text. This clearly delineates the region and prevents ambiguity about where the time jump ends. Nesting is prevented by checking parent nodes in `insertTimeBreak`.

### Right-click context menu
The editor has a custom context menu (`handleContextMenu` in Editor.svelte) that adapts based on what was right-clicked: entity words get "Go to definition", "Find references", and "Set state value" (creates a state checkpoint); misspelled words get "Spelling & Synonyms..." and "Add to dictionary"; all words get "Synonyms..." and entity creation options; "Add a note" is always available. The WordModal component is a large modal with flowing pill grids for both spelling suggestions and synonyms.

### Decoration coexistence (five independent plugin systems)
Five separate ProseMirror plugins manage decorations/interactions independently: `entityHighlightKey` for entity highlights, `spellCheckKey` for red squiggly underlines, `commentDecoKey` for comment highlights, `stateMarkerDecoKey` for state marker click handling, and `debugDecoKey` for debug/analysis overlays (dialogue detection highlighting). The `applyingDecorations` flag prevents entity/spell decorations from triggering infinite loops. Each plugin has its own PluginKey and state management. Debug decorations are applied via `applyDebugDecorations()` and cleared by passing an empty array.

### Entity notes use Y.Doc relative positions
Entity notes (pinned excerpts) store Y.Doc relative positions (`y_start BLOB`, `y_end BLOB`) instead of text. These positions are created via `absolutePositionToRelativePosition()` from `y-prosemirror` and survive document edits — if text is inserted within a pinned range, the range expands to include it. Text is resolved live at display time via `relativePositionToAbsolutePosition()`. Notes for chapters not currently open show "Open chapter to view". Editor.svelte exposes `createRelativePositions()`, `resolveRelativePositions()`, and `getTextBetween()` for this.

### Dialogue detection (live highlighting)
The "Dialogue Detection" tool in AnalysisPanel highlights all detected dialogue in the editor using the `debugDecoKey` ProseMirror plugin. Detection runs entirely in JS using `buildTextMap` text — no Rust round-trip, positions are guaranteed correct. Quote marks get a darker highlight, inner dialogue text gets a lighter highlight. The highlighting recalculates live on edit (300ms debounce) so authors can fix mismatched quotes and see the detection update in real-time.

### Charts use Canvas 2D (no library)
All charts across the app (chapter analysis, heatmap, pacing waveforms) use raw `canvas.getContext('2d')` calls. No chart library. The design system uses: `#faf8f5` background, `#2d6a5e` teal for primary data, `#d97706` amber for secondary, `#6b6560` for labels, Libre Baskerville for titles, Source Sans 3 for UI text.

### Writing stats tracking
On every content save (500ms debounce), the project page computes word count delta and calls `logWritingActivity()`. The Rust side updates `daily_stats` atomically. Active time is computed from gaps between consecutive activities (capped at session_gap_minutes).

### Settings storage
App-level settings (projects folder, panel widths) are stored in `settings.json` in Tauri's app data dir via `tauri-plugin-fs`. Project-level settings (writing goals, entity data) are in the `.iwe` SQLite file.

## Common Patterns

### Shared text utilities (`text_utils.rs`) — CRITICAL

All text analysis that needs dialogue or sentence boundaries MUST use the shared functions in `text_utils.rs`. Do not implement your own splitting logic.

**`extract_dialogue(plain) → Vec<DialogueSpan>`** — Finds all dialogue spans in plain text. Handles straight quotes, curly quotes, guillemets, CJK brackets, and mixed quote styles. Uses heuristics to disambiguate straight `"` as opener vs closer (checks if next char is a letter). Caps at 500 chars per span to prevent runaway mismatched quotes. Used by: dialogue search, chapter analysis (dialogue/narrative split), adverb analysis.

**`extract_sentences(plain) → Vec<Sentence>`** — Robust sentence extraction. Handles abbreviations (Mr. Mrs. Dr. etc.), ellipsis, decimal numbers, initials (J.K.), multiple punctuation (!! ??), closing quotes after punctuation. Returns text, char_start, char_end, word_count per sentence. Used by: chapter analysis, pacing analysis, similar phrasing, heatmap.

**`count_words(text) → usize`** — Word counting utility.

### Analysis tools (`analysis.rs`)

All analysis commands live in `analysis.rs`, separate from the core entity scanner. This keeps the scanner focused on Aho-Corasick matching.

**Inline tools** (results in the right panel, clickable to navigate):
- `word_frequency` — word repetition counts with per-chapter breakdown
- `find_similar_phrases` — sentence similarity detection via Jaccard + LCS
- `adverb_analysis` — finds adverbs in dialogue attribution near speech verbs

**Popup window tools** (launch separate windows):
- `chapter_analysis` — word counts, dialogue/narrative split, sentence stats, vocabulary density
- `generate_heatmap` — entity mention grids (chapter-level and sentence-level)
- `pacing_analysis` — sentence length waveforms per chapter with navigation

**Analysis tool selector** in AnalysisPanel uses a grouped dropdown menu (not tabs) to scale to many tools. Groups: Repetition, Style, Overview. To add a new tool: add an entry to the `analysisTools` array, add the `{:else if subTab === 'mytool'}` view, and add the Rust command if needed.

### Adding a new Tauri command

1. Add the function to the appropriate Rust file: `db.rs` (data), `scanner.rs` (entity scanning), `analysis.rs` (analysis tools), `text_utils.rs` (shared text utilities), `ydoc.rs` (Y.Doc operations)
2. Add the `#[tauri::command]` wrapper in `src-tauri/src/lib.rs`
3. Register it in the `invoke_handler![]` macro in `lib.rs`
4. Add the JS wrapper in `src/lib/db.js`
5. Call it from Svelte components via `import { myFunction } from '$lib/db.js'`

### Adding a new popup window

1. Create `src/routes/mywindow/+page.js` (with `prerender = false, ssr = false`)
2. Create `src/routes/mywindow/+page.svelte`
3. Add `"mywindow*"` to the `windows` array in `src-tauri/capabilities/default.json`
4. Launch with `new WebviewWindow('mywindow-' + Date.now(), { url: '/mywindow', ... })`
5. Add `:global(html), :global(body) { overflow: auto !important; height: auto !important; }` to enable scrolling
6. If the window reads chapter content, convert Y.Doc bytes to HTML via `yDocToProsemirrorJSON()` + `generateHTML()`

### Adding a new analysis tool

1. Add the tool entry to the `analysisTools` array in `AnalysisPanel.svelte` (grouped by category)
2. Add state variables for the tool in the `<script>` section
3. Add the `{:else if subTab === 'mytool'}` view in the template
4. If it needs Rust processing, add the command to `analysis.rs` and register it
5. For popup tools: create a route in `src/routes/`, add window label to `capabilities/default.json`
6. For inline tools with navigation: pass `ongotochapter(chapterId, searchText, charPositionHint)`

### Adding a new right panel tab

1. Add tab state value to `rightPanelTab` in `+page.svelte`
2. Add the tab button in the `.panel-tabs` div
3. Create the panel component in `src/lib/components/`
4. Add the `{:else if rightPanelTab === 'mytab'}` conditional render

## Design System

**Colors:** Warm whites (#faf8f5 sidebar, #fffef9 paper), deep literary teal accent (#2d6a5e), warm grays for text hierarchy. Entity type colors: character=teal, place=brown, thing=purple. Comment highlights: warm yellow (#ffd450 at varying opacity).

**Typography:** Libre Baskerville (serif) for prose/titles, Source Sans 3 (sans) for UI. Loaded via Google Fonts in `theme.css`.

**Entity colors** are user-customizable per entity with a color picker. Defaults by type. Color is used for editor highlights, heatmap cells, entity dots, and search result highlighting.

## User Preferences (from memory)

- **Vanilla JS only** — no TypeScript. All files are `.js`, no `lang="ts"` in Svelte.
- **Do it right** — Rust-side processing preferred over JS workarounds. The full DB migration to rusqlite was done specifically because passing data through JS was "not the right way."
- **Author-focused UX** — features should be framed for writers, not developers. "Scene Heading" not "H3". Word types `{verb}` not regex jargon.

## Spec

The full feature vision is in `spec.md` at the project root. It describes the MVP scope, post-MVP roadmap, entity relationship search, analytics, and IDE-inspired features.
