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
- **Editor:** TipTap 3 (ProseMirror-based) with custom nodes (`noteMarker`, `stateMarker`, `timeBreak`)
- **Document Model:** Yjs (`yrs` in Rust, `yjs` + `y-prosemirror` in JS) — every character has a permanent identity
- **Backend:** Tauri v2 (Rust), rusqlite with bundled SQLite
- **Scanner:** Aho-Corasick (Rust) for blazing-fast multi-pattern entity matching
- **Semantic Search:** ONNX Runtime (`ort` crate) + `tokenizers`, running all-mpnet-base-v2 on-device (no cloud, no API keys)
- **Publishing / Layout:** Typst (`typst` crate) for the Format editor's SVG preview, served via a custom `iwe://` URI scheme
- **Quick PDF export (toolbar):** `genpdf` with embedded Liberation Serif fonts (separate code path from the Typst-based Format editor)
- **DOCX export:** `docx` npm package
- **Styling:** Bootstrap 5 + Bootstrap Icons + custom CSS theme (`src/lib/theme.css`)
- **Drag & Drop:** svelte-dnd-action (used in entity excerpts, kanban, time flow, format pages)

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
- `src-tauri/src/ydoc.rs` — `load_doc()`, `extract_plain_text()`, `extract_text_with_breaks()`, `extract_text_for_format()`, `extract_chapter_blocks_from_bytes()`, `word_count()`
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

**Notes panel** (right panel tab): List → detail pattern. Clicking a note navigates to it and shows the detail view with editable textarea. "Update highlight" button (only visible when text is selected in editor) moves the note to the current selection. Two-step inline delete confirmation. The panel ALSO shows a read-only "Kanban Notes" section listing any `chapter_planning_notes` rows for the current chapter (the same rows edited in the Kanban → Chapters board) so writers see planning context alongside inline notes.

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
- `src-tauri/capabilities/default.json` — includes `"timeflow*"`, `"pacing*"`, `"readability*"`, `"paragraphs*"`

### Navigation / Jump-to-Position (CRITICAL — read this carefully)

`handleGoToChapter(chapterId, searchText, positionHint)` in the project page is the **ONE central function** for ALL click-to-navigate-and-highlight across the entire app. Every feature that lets the user click a result and jump to a position in the editor MUST use this function. Do not create alternative navigation paths.

**Used by:** find-all-references, text search, dialogue search, relationship search, similar phrasing, word frequency browser, cluster finder, entity detection go-to, pinned excerpt clicks, pacing analysis, adverb analysis, readability analysis, paragraph length analysis.

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

### Windows architecture — two kinds of "other view"

There are two ways a non-editor view can be shown, and it's important not to confuse them:

**1. Project subroutes (inline, same window, share the project layout + open DB handle):**
Kanban, Editor, Palettes, Stats, Time Flow, Formatting — these are nav tabs in `src/routes/project/[filename]/+layout.svelte` and live at `/project/[filename]/{kanban,palettes,stats,timeflow,format}`. They're reached via `<a href>` inside the shell. They see the same SQLite connection that `open_project` set up.

**2. Popup windows (separate Tauri `WebviewWindow`, independent process, open their own DB view):**
`heatmap`, `chapters`, `pacing`, `readability`, `paragraphs`, `print`, `import`, `analyse` — launched via `new WebviewWindow('label-' + Date.now(), { url: '/path', ... })` from `@tauri-apps/api/webviewWindow`. Each window's label prefix must be listed in `src-tauri/capabilities/default.json` under `windows`. They read the DB directly via Tauri commands.

**Cross-window events:** Popup analysis windows can navigate the main editor via `emitTo('main', 'navigate-to-position', payload)` from `@tauri-apps/api/event`. The project layout listens, forwards to a `window` CustomEvent `iwe-navigate-to-position`, and the editor page's listener calls `handleGoToChapter`. Payload: `{ chapterId, searchText, charPosition }`.

**Four independent export paths — keep them straight:**
- `/print/+page.svelte` (popup) is NOT Typst — it converts Y.Doc → ProseMirror JSON → HTML via `yDocToProsemirrorJSON` + `generateHTML`, applies A4 or Book CSS, and triggers `window.print()`. Fast, rough, zero layout control.
- The toolbar `Export → PDF` uses `genpdf` in Rust (`lib.rs:686-823`) with embedded Liberation Serif, A4 or 5×8 book preset. Also independent of Typst.
- The Format editor's **print** profiles use Typst: `format::compile_preview` compiles a cached `PagedDocument`, exposes pages as SVG via the custom `iwe://localhost/preview/page/{n}.svg` URI scheme (see `lib.rs:1278-1312`), and exports via `format::export_format_pdf` → `typst_pdf::pdf(...)`. This is the "publication-quality" path.
- The Format editor's **ebook** profiles use `epub::export_epub` in Rust (`epub-builder` crate, stateless) — the frontend pre-converts chapters to HTML via `yDocToProsemirrorJSON` + `generateHTML` with an `epubExtensions` set. Ebook preview is a device-frame HTML render on the frontend (`generateEbookPreview`), never touches Typst. See the Format editor section below for details.

## Project Structure

```
src/
├── routes/
│   ├── +page.svelte              # Home shell (sidebar: Projects / Word Palettes / Settings)
│   ├── project/[filename]/
│   │   ├── +layout.svelte        # Project shell: top toolbar (6 nav tabs + Export dropdown)
│   │   ├── +page.svelte          # Editor view (3 resizable panels: ChapterNav / Editor / right panel)
│   │   ├── kanban/               # Kanban board (3 modes: chapters / entities / freeform)
│   │   ├── palettes/             # Read-only palette browser for the current project
│   │   ├── stats/                # Writing stats dashboard (contribution calendar, growth, hourly)
│   │   ├── timeflow/             # Time Flow Manager — drag sections into story-time order
│   │   └── format/               # Typst-based Format & Layout editor with live SVG preview
│   ├── heatmap/                  # Entity heatmap (POPUP) — chapter grid + sentence timeline
│   ├── chapters/                 # Chapter analysis (POPUP) — 4 charts + 12-col breakdown table
│   ├── pacing/                   # Pacing (POPUP) — whole-book sentence-length waveform
│   ├── readability/              # Flesch-Kincaid (POPUP) — overview + per-chapter sentence detail
│   ├── paragraphs/               # Paragraph length variation (POPUP)
│   ├── print/                    # Print preview (POPUP) — HTML + window.print(), NOT Typst
│   ├── import/                   # Manuscript import wizard (POPUP) — DOCX + EPUB
│   └── analyse/                  # Dev-only batch analysis (POPUP) — runs all 6 analyses and saves to the famous-books library
├── lib/
│   ├── db.js                     # ALL Tauri command wrappers + settings.json helpers
│   ├── entityHighlight.js        # ProseMirror decoration plugins (entities + spellcheck + debug)
│   ├── noteMarker.js             # NoteMarker TipTap node + comment highlight decorations
│   ├── stateMarker.js            # StateMarker TipTap node (entity state checkpoints)
│   ├── timeBreak.js              # TimeBreak TipTap wrapping node (time jump regions)
│   ├── ydoc.js                   # Y.Doc lifecycle (create, encode, destroy)
│   ├── export.js                 # DOCX/HTML/TXT export helpers (PDF paths are Rust-side)
│   ├── theme.css                 # Design system CSS variables
│   ├── toast.js                  # Toast notification store
│   └── components/
│       ├── Editor.svelte         # TipTap editor + toolbar + context menu + comments + state markers + time breaks + typewriter mode
│       ├── WordModal.svelte      # Combined spelling & synonym modal (flowing pill grid)
│       ├── PalettePickerModal.svelte # Modal for picking a replacement word from active palettes
│       ├── EntityPanel.svelte    # Entity CRUD + detection + references + view (excerpts/notes/state tabs)
│       ├── NotesPanel.svelte     # Inline notes + read-only Kanban notes for current chapter
│       ├── ChapterNav.svelte     # Chapter list: rename, delete, restore-from-archive modal
│       ├── SearchPanel.svelte    # Text / Dialogue / Relationship / Descriptive (semantic) search
│       ├── AnalysisPanel.svelte  # Analysis tool selector — 11 tools across 4 groups
│       ├── FontPicker.svelte     # System font picker with in-face previews (calls list_system_fonts)
│       ├── PageContentEditor.svelte # Rich text editor for free-form format pages (with image support)
│       ├── TocPageEditor.svelte    # TOC page settings editor (title, leader style)
│       ├── KanbanCardModal.svelte # Create/edit card modal used by Kanban board
│       ├── WordPalettes.svelte   # Full palette CRUD (used by the home sidebar)
│       ├── Toasts.svelte         # Toast renderer
│       └── format/               # Settings subpanels for the Format editor "Custom" mode:
│           ├── ChapterHeadings.svelte
│           ├── ParagraphSettings.svelte
│           ├── HeadingsSettings.svelte
│           ├── BreaksSettings.svelte
│           ├── PrintLayoutSettings.svelte
│           ├── TypographySettings.svelte
│           ├── HeaderFooterSettings.svelte
│           └── TrimSettings.svelte

src-tauri/
├── src/
│   ├── main.rs                   # Entry point
│   ├── lib.rs                    # Tauri commands (all #[tauri::command] wrappers + genpdf export + backup helpers)
│   ├── db.rs                     # Database schema + all queries + migrations
│   ├── scanner.rs                # Aho-Corasick scanner, entity detection, text_search, dialogue_search, relationship_search, find_references
│   ├── analysis.rs               # word_frequency, find_similar_phrases, generate_heatmap, chapter_analysis, pacing_analysis, adverb_analysis, readability_analysis, paragraph_length_analysis
│   ├── text_utils.rs             # Shared: extract_dialogue, extract_sentences, count_words (THE source of truth)
│   ├── ydoc.rs                   # Y.Doc load/encode, text extraction, extract_time_sections, locate_state_markers_in_sections
│   ├── spellcheck.rs             # Hunspell-compatible parser for en_US + en_GB, custom words, suggestions
│   ├── synonyms.rs               # Moby Thesaurus II lookup (in-memory, suffix-aware)
│   ├── syllable_data.rs          # AUTO-GENERATED phf map of word→syllable counts
│   ├── wordlists.rs              # Speech-verb / adverb / etc. lists for analysis
│   ├── semantic.rs               # ONNX session + tokenizer, embedding index, semantic_search, indexing events
│   ├── format.rs                 # Typst-based Format editor: FormatState, compile_preview, render_page_svg, list_system_fonts (feature-gated: cfg(feature = "format"))
│   ├── epub.rs                   # Stateless EPUB export via epub-builder crate — consumed by Format editor's ebook profiles
│   ├── import.rs                 # DOCX + EPUB parse → flat blocks → 5-strategy chapter break detection
│   ├── palettes.rs               # Word Palettes: separate SQLite DB in app data dir, 21 palette commands
│   ├── famous_books.rs           # Comparative-works library: separate SQLite DB in app data dir
│   ├── _emotions_ag.txt          # Seeded palette: emotions a–g with physical manifestations
│   ├── _emotions_gn.txt          # Seeded palette: emotions g–n
│   ├── _emotions_oz.txt          # Seeded palette: emotions o–z
│   ├── _sensory.txt              # Seeded palette: sight/sound/touch/smell/taste
│   ├── _setting_dialogue.txt     # Seeded palette: weather & setting observations
│   └── _movement.txt             # Seeded palette: motion/movement words
├── fonts/
│   └── LiberationSerif-*.ttf     # Embedded fonts for genpdf toolbar export AND Typst fallback
├── generate_syllables.py         # One-time script: CSV → syllable_data.rs (phf map)
├── resources/
│   ├── dictionaries/             # Hunspell en_US + en_GB (.dic/.aff) — GITIGNORED, embedded via include_str! at build
│   ├── mthesaur.txt              # Moby Thesaurus II — GITIGNORED, embedded via include_str! at build
│   ├── syllables-list.csv        # Dev-only, compiled into syllable_data.rs (not shipped)
│   └── models/                   # ONNX semantic model directory
│       ├── model.onnx            # all-mpnet-base-v2 ONNX export (large — may be gitignored locally)
│       └── tokenizer.json        # HuggingFace tokenizer config
├── capabilities/default.json     # Tauri permissions + popup window label allowlist
└── Cargo.toml
```

**IMPORTANT — the `resources/` directory is partially gitignored.** `dictionaries/` and `mthesaur.txt` are listed in `.gitignore` because they're too large for git, and must be provided locally before `cargo build` will succeed (the Rust code uses `include_str!` on them at compile time). `resources/models/model.onnx` is similarly expected to exist on disk but isn't tracked. When setting up a fresh checkout, ensure these files are present or builds will fail.

## Database Schema

Three databases exist at runtime:

1. **Per-project `.iwe` file (SQLite)** — all of the writer's content. Opened via `open_project` command, connection stored in `AppState.db`.
2. **`palettes.db` (SQLite, in app data dir)** — shared Word Palettes across all projects. Managed in `palettes.rs`, state in `PaletteState`.
3. **`famous_books.db` (SQLite, in app data dir)** — the comparative-works library used by the Analysis panel's "Comparative Works" tab and overlays. Managed in `famous_books.rs`, state in `LibraryState`.

### Per-project `.iwe` schema (tables defined in `db.rs::init_schema`)

**Writing core:**
- **chapters** — id, title, content (BLOB — Yjs state), sort_order, created_at, updated_at, **deleted** (soft-delete flag — restore via `restore_chapter`)
- **entities** — id, name, entity_type CHECK('character'|'place'|'thing'), description, color, visible
- **aliases** — entity_id, alias
- **entity_fields** — key-value custom fields *(schema exists, no UI yet)*
- **ignored_words** — words excluded from entity detection
- **custom_words** — spell checker custom dictionary (word, source='user'|'entity'; auto-synced from entities)

**Annotation / linking:**
- **entity_notes** — pinned excerpts stored as Yjs relative positions (y_start BLOB, y_end BLOB), with chapter_id and sort_order. Ranges expand when text is inserted inside them.
- **entity_free_notes** — free-form note cards per entity with title, text, sort_order (the rows that Kanban "Entities" board also edits)
- **comments** — chapter_id, note_text (position lives in the Y.Doc as a `noteMarker` atom node)
- **state_markers** — entity_id, chapter_id, note (position lives in the Y.Doc as a `stateMarker` atom node)
- **state_marker_values** — marker_id, value_type='fact'|'entity_ref', fact_key, fact_value, ref_entity_id, ref_active
- **time_section_order** — chapter_id, section_index, label, story_order (global sort key for Time Flow Manager; when empty the system falls back to chapter sort_order × 1000 + section_index)

**Planning / kanban:**
- **chapter_planning_notes** — chapter_id, title, description, sort_order (shown in the Kanban "Chapters" board and the Notes panel's read-only Kanban section)
- **kanban_columns** — title, sort_order (freeform board only)
- **kanban_cards** — column_id, title, description, sort_order (freeform board only)

**Semantic search:**
- **semantic_embeddings** — chapter_id, granularity, segment_text, char_start, char_end, embedding (BLOB)
- **semantic_index_status** — chapter_id PK, content_hash, indexed_at (used to detect dirty chapters)

**Format / layout (Typst editor):**
- **format_profiles** — name, target_type ('print'|'ebook'), trim dimensions, margins, font_body, font_size_pt, line_spacing, plus 8 JSON category columns: `chapter_headings_json`, `paragraph_json`, `headings_json`, `breaks_json`, `print_layout_json`, `typography_json`, `header_footer_json`, `trim_json`
- **format_pages** — page_role ('free-form'|'toc'|future types), title, content (PM JSON for free-form, settings JSON for toc), position ('front'|'back'), sort_order, include_in, vertical_align, ebook_metadata_tag (semantic role for EPUB export, e.g. 'copyright', 'dedication'). Shared across all profiles; the Format editor uses `format_page_exclusions` to hide specific pages per profile.
- **format_page_exclusions** — (page_id, profile_id) composite PK

**Stats / history / misc:**
- **writing_activity** — per-save log (timestamp, chapter, chapter_words, manuscript_words, words_delta)
- **daily_stats** — aggregated daily (date PK, words_added, words_deleted, net_words, active_minutes, chapters_touched)
- **writing_settings** — singleton: daily_goal, session_gap_minutes, **backup_interval_minutes**, **last_backup_at**
- **nav_history** — chapter_id, scroll_top, cursor_pos (bounded to ~100 entries; truncated forward on new navigation like a browser)
- **project_settings** — generic key/value store (e.g. `comparative_book_id` for the Comparative Works benchmark)

Migrations are handled in `init_schema()` with `CREATE TABLE IF NOT EXISTS` for idempotent creation and `ALTER TABLE ... ADD COLUMN` checks for backward compatibility. There's also a one-off migration in `init_schema()` that drops the old `profile_id` column from `format_pages` and deduplicates rows — see `db.rs:360-400`. During active development, schema changes may require deleting the `.iwe` file and starting fresh.

## Feature Subsystems

These are the major subsystems not already covered above. Each one has its own route/component/Rust module and can be understood in isolation.

### Home shell — `src/routes/+page.svelte`
Three sidebar views: **Projects**, **Word Palettes** (renders `WordPalettes.svelte`), **Settings**. First-run flow: pick a projects folder (persisted via `getProjectsDir`/`setProjectsDir` in `db.js`). Projects view lists `.iwe` files from the chosen folder, buttons: New Manuscript, Import…, Refresh, Open Folder (reveals in OS), Change Folder. Delete has a "type `delete` to confirm" modal. A dev-only ⚗ button on each project row launches the `/analyse` popup window (visible only when `import.meta.env.DEV`). Settings view covers: Interface text size (UI scale, stored as `uiScale` in settings.json, applied via root font-size in `+layout.svelte` — scales everything since nearly all font-size declarations use `rem`), Dictionary language (en_US / en_GB), Typewriter mode, Descriptive Search indexing delay (seconds), Projects folder, Backup interval (minutes), Measurement unit (in/mm), plus license credits for ONNX + all-mpnet-base-v2.

### Semantic (Descriptive) Search — `src-tauri/src/semantic.rs`
On-device ONNX embeddings. `SemanticState` holds a lazy-initialized `ort::Session` and `tokenizers::Tokenizer` (512-token truncation, 2 intra-threads). Model files are resolved from `resources/models/model.onnx` + `tokenizer.json` via `resolve_model_dir`, falling back to the Cargo manifest dir for dev. Commands: `mark_chapter_dirty`, `run_semantic_indexing`, `rebuild_semantic_index`, `semantic_search`, `get_semantic_index_status`. Emits Tauri events `semantic-index-started`, `semantic-index-progress`, `semantic-index-updated` which the project page and SearchPanel both listen for. Auto-indexes a chapter after configurable inactivity delay (default 30s, stored in `settings.json` as `semanticIndexDelay`, 0 disables auto). Embeddings stored in `semantic_embeddings` BLOB column, dirtiness tracked via `semantic_index_status.content_hash`. **Backups strip both semantic tables before VACUUM** to keep backup files small — see `lib.rs:94-101`.

**UI granularity:** only **Sentences** and **Paragraphs** are exposed in `SearchPanel.svelte`. There is no "Chapter" option — do not add one without extending the Rust side.

### Word Palettes — `src-tauri/src/palettes.rs`
**Separate SQLite database** (`palettes.db` in app data dir) so palettes are shared across all projects. Managed via its own `PaletteState` struct and 21 Tauri commands (see `lib.rs:1419-1440`: `get_palettes`, `create_palette`, `update_palette`, `delete_palette`, `toggle_palette`, `copy_palette`, plus groups, sections, entries, and `search_word_groups` / `search_palette_entries`). **Seeded content** comes from six bundled `.txt` files (`_emotions_ag.txt`, `_emotions_gn.txt`, `_emotions_oz.txt`, `_sensory.txt`, `_setting_dialogue.txt`, `_movement.txt`) that are parsed into palettes on first init. Two UI entry points: the `WordPalettes.svelte` component on the home sidebar (full CRUD) and the `/project/[filename]/palettes` subroute (read-only browser scoped to active groups). The editor's right-click menu also exposes `PalettePickerModal.svelte` for swapping a selected word with one from an active palette.

### Kanban — `src/routes/project/[filename]/kanban/+page.svelte`
Three board modes selected by toggle: **Chapters / Entities / Freeform**. Each mode uses different tables:
- **Chapters mode** — columns = `chapters` (respects sort order); cards = `chapter_planning_notes`; column reorder calls `reorder_chapters`.
- **Entities mode** — columns = `entities` (sorted by name, order not persisted); cards = `entity_free_notes`; columns show entity dot + type + count; inline entity edit modal with name/type/color/aliases/description.
- **Freeform mode** — columns = `kanban_columns`; cards = `kanban_cards`; fully user-defined.

Drag-and-drop within columns and between columns (cards share a DnD group). Columns also reorderable. **Hand-grab horizontal scrolling**: mousedown on empty board background + drag to scroll horizontally, useful on wide boards. Card modal is `KanbanCardModal.svelte`.

### Writing Stats — `src/routes/project/[filename]/stats/+page.svelte`
Project subroute (NOT a popup). Header inputs for daily_goal (50–10000 words) and session_gap_minutes (5–120) auto-save on blur via `update_writing_settings`. Six stat cards at the top: Words Today (with goal progress bar), Day Streak, Minutes Active, Avg Words/Day, Best Day, Days Writing. Four Canvas 2D charts:
1. **Writing Calendar** — GitHub-style contribution grid for a selectable year (back/forward buttons, can't go past current year). Hover tooltip shows day + word count. Clicking a cell opens a fifth chart:
2. **Hourly Breakdown** — 24-bar chart for the selected day with green (added) / red (deleted) bars. Summary: net words, added, deleted, active hours.
3. **Last 30 Days** — daily net words with daily-goal dashed line, darker bars where goal was met.
4. **Manuscript Growth** — cumulative word count over time.

Data source: `logWritingActivity` is called on every 500ms-debounced content save from the editor page; Rust aggregates into `daily_stats`.

### Format & Layout editor — `src/routes/project/[filename]/format/+page.svelte` + `src-tauri/src/format.rs` + `src-tauri/src/epub.rs`

The "publication-quality" path. Project subroute. Two-column layout: **preview column (left) + profile sidebar (right, resizable 220–600px, width persisted to `settings.json` as `formatSidebarWidth`)**. Read this entire section before touching any formatting code — the subsystem has a lot of load-bearing details.

#### Feature gating: the whole Typst path is `#[cfg(feature = "format")]`

The real implementation in `format.rs` is wrapped in `mod real` inside `#[cfg(feature = "format")]`. When the feature is off, lightweight stub types + stub commands compile instead (`format.rs:1901-1957`), which lets `lib.rs` build without pulling in the Typst crate family. Activated in `Cargo.toml` as `format = ["dep:typst", "dep:typst-pdf", "dep:typst-svg", "dep:typst-kit"]`. Neither `format` nor `semantic` is in `default`, so plain `cargo build` skips both. **Tauri dev/build must pass `--features format` explicitly** or you'll get "Format features not enabled" at runtime. Use `npm run tauri:full` (dev with `--features semantic,format`) or `npm run tauri:build` (release build with both features). Typst builds are slow — the stub mode keeps everyday dev fast.

Typst crates used (all pinned at `0.14`): `typst` (core, `World` trait, `PagedDocument`), `typst-pdf` (PDF export), `typst-svg` (per-page SVG), `typst-kit` (system font discovery + package storage, `default-features = false, features = ["fonts", "packages"]`). The `epub-builder = "0.8"` crate is **not** feature-gated and always compiles.

#### How the preview works, end to end

1. **`compile_preview(profile_id)`** (format.rs:1711-1826) is the entry point. It:
   - Takes the `AppState.db` mutex at the start and **holds it for the whole compile** (DB load → Y.Doc extract → markup build → Typst compile). This is 4 serial phases on one mutex; any other DB operation blocks during a compile. Architectural gotcha to be aware of.
   - Early returns for ebook: if `profile.target_type == "ebook"`, clears `FormatState.document = None` and returns `CompileResult { page_count: 0, ... }`. Typst is never invoked for ebook profiles. Ebook preview is rendered entirely on the frontend as HTML (see below).
   - Loads pages via `list_format_pages`, filters via `list_excluded_page_ids_for_profile`, loads `list_chapters`, then reads four `project_settings` keys inline via `query_row`: `book_title`, `author_name`, `series_name`, `series_number`.
   - Extracts chapter content via `ydoc::extract_chapter_blocks_from_bytes` — returns `Vec<ChapterBlock>` (structured paragraphs with inline formatting marks + scene breaks) using the yrs `XmlTextRef::diff()` API to read formatting attributes directly from the Y.Doc. Each `ChapterBlock::Paragraph` contains `Vec<FmtSpan>` with text + boolean flags for bold/italic/underline/strike/superscript/subscript + optional font size/family. `ChapterBlock::SceneBreak` replaces the old `"* * *"` sentinel pattern. The Typst builder converts spans to markup via `apply_fmt_span()` / `spans_to_typst()`.
   - Calls `build_typst_markup(...)` → returns `(String markup, Vec<String> section_ids, ImageMap images)`.
   - Lazy-inits the font cache via `FormatState::get_or_init_fonts()` on first call.
   - Calls `compile_document(...)` which builds a fresh `IweWorld` per compile and runs `typst::compile::<PagedDocument>(&world)`. Always dumps the full markup to `$TEMP/iwe_typst_debug.typ` on every compile (success or failure) for debugging. On error, returns a descriptive error with line/column context — check the dump file first when debugging markup issues.
   - After compile, `resolve_section_pages(&document, &section_ids)` walks the Typst introspector using `Label::new()` on each `"iwe-ch-{id}"` / `"iwe-fp-{id}"` label to map section → 0-indexed page number. Returned as `section_pages: HashMap<String, usize>` for the frontend's scroll-to-section feature.
   - Stores the new `PagedDocument` in `format_state.document` (behind a `Mutex<Option<PagedDocument>>`). **There is no generation counter in Rust** — the "generation" in the URL is a frontend JS variable.

2. **The `iwe://` URI scheme** (lib.rs:1278-1312) is registered once before `invoke_handler`. It handles exactly one route: `/preview/page/{N}` (with optional `.svg` suffix stripped). The render call is wrapped in `std::panic::catch_unwind(AssertUnwindSafe(...))` because `typst_svg::svg()` can panic on certain content (font issues, edge-case glyphs) and this callback runs inside a webview2 COM handler (`extern "C"`) where unwinding panics abort the process. Panics return a 500 response instead. Frontend requests `http://iwe.localhost/preview/page/{i}.svg?v={generation}` (Tauri normalizes custom schemes to `http://{scheme}.localhost/...`). `render_page_svg` is a plain public function, **not** a Tauri command. When adding new `iwe://` asset routes in the future, extend the existing handler — Tauri only allows one `register_uri_scheme_protocol` call per scheme.

3. **Lazy page loading** (`format/+page.svelte:249-310`): the frontend renders `pageCount` page wrappers in a **2-column CSS grid spread layout** (verso/recto side-by-side, like an open book). Page 1 (recto) sits alone on the right via `grid-column: 2`; subsequent pages fill left-right pairs. Verso pages align toward the spine (right), recto pages toward the spine (left). Placeholders are sized via `width: {trim_width_in * 72}px; aspect-ratio: {w}/{h}` so the scroll height is correct before any SVG loads.
   - An `IntersectionObserver` with `root: previewContainer, rootMargin: '200px 0px 200px 0px', threshold: 0` tracks `visibleSet` (non-reactive Set).
   - On scroll, `scrolling = true` + 150ms debounce (`SCROLL_IDLE_MS`) to `commitVisible()`.
   - `commitVisible()` computes `[min(visible)-2, max(visible)+2]` (`VISIBLE_BUFFER = 2`) and adds any new indices to `loadedPages` ($state Set). Svelte reactivity then sets `<img src>` on those pages only — pages outside the buffer stay as placeholders forever until scrolled near.
   - A `$effect` on `[pageCount, compileGeneration]` calls `queueMicrotask(() => setupObserver())` so the observer re-attaches to fresh DOM nodes after every compile.
   - `compileGeneration` is a frontend counter incremented on each successful compile, used as the `?v=` cache-bust query param so the browser re-fetches even if the URL path is identical.
   - Scroll restoration: `compileAndShow()` reads `previewContainer.scrollTop` and `loadSavedScroll(profileId)` (from `project_settings`), waits TWO `requestAnimationFrame` calls after compile (to let Svelte commit placeholders and let the browser compute layout), then jumps. Per-profile scroll persistence is intentional.

4. **Timing bar** (bottom of preview) shows five values from the `CompileTiming` struct: `total_ms`, `db_load_ms`, `ydoc_extract_ms`, `markup_build_ms`, `typst_compile_ms`, plus `page_count`. Debug aid — keep it.

5. **Compile triggering** — there is **NO debounce** on compile. `compileAndShow()` is called directly after: initial load, route navigation, profile switch, every Custom subpanel save (via `handleCustomSettingChange`), page add/edit/delete/reorder, profile-inclusion pill toggle (if affected profile is active), profile create/paste/delete. Every trigger fires a full recompile. The `rendering` flag shows a spinner while in flight; overlapping compiles are not debounced or cancelled.

#### Profile management UI (top of sidebar)

Dropdown + pencil-rename (inline `<input>`, commits on blur/Enter via `updateFormatProfile` preserving all other scalar fields) + **Copy settings** (snapshots the active profile as `formatClipboard = { ...activeProfile }` and persists to `settings.json`) + **Paste settings** (calls `pasteFormatProfileSettings(activeProfile.id, formatClipboard)`; disabled when clipboard is null or its id matches the active profile) + **New profile** modal (optionally duplicate from an existing profile + pick Print/Ebook target) + **Delete** (inline Yes/No confirmation, auto-picks a remaining profile before calling `deleteFormatProfile`).

#### Sidebar modes — FOUR modes, not five

`SIDEBAR_MODES = [pages, themes, custom, export]`. **There is no standalone "Target" mode anymore** — trim size / target type selection was moved into the Custom → "Target Format" subpanel (`TrimSettings.svelte`). If you see docs or comments referencing a "Target" sidebar tab, they're out of date.

**Pages mode:**
- **Page types:** Pages have a `page_role` that determines their type and editor. Currently two types: `'free-form'` (rich text via PageContentEditor) and `'toc'` (auto-generated table of contents via TocPageEditor). Extensible — add new types to `PAGE_TYPES` array and their editor routing in `openPageEditor()`.
- **Add page modal:** The `+` button on Front/Back Matter opens a modal to choose a page type. This replaces the old tag-based page creation system.
- **Ebook metadata tags (ebook profiles only):** A tag bar with 17 `EBOOK_TAGS` (half-title, title, copyright, etc.) appears only for ebook profiles. These annotate pages with semantic roles for EPUB export (`epub:type`). Stored in `format_pages.ebook_metadata_tag` column. Click a tag to arm → click a page to assign. Used tags show ×; clicking × clears the tag. Tags are independent of page type — a free-form page can be tagged as "copyright", etc.
- Three sections rendered with `svelte-dnd-action`: **Front Matter** (draggable, `position: 'front'`), **Chapters** (locked, iterates `chapters`), **Back Matter** (draggable, `position: 'back'`). Front/back use `handleFrontFinalize`/`handleBackFinalize` which extract ordered ids and call `reorderFormatPages`. Clicking a non-armed page calls `scrollToSection('iwe-fp-{id}')` which looks up `sectionPages` (returned by compile) and smooth-scrolls. Clicking a chapter calls `scrollToSection('iwe-ch-{chapter_id}')`.
- **Profile-inclusion pills**: below each page, one pill per profile. `isPageIncludedIn(pageId, profileId) = !exclusions.some(e => e.page_id === pageId && e.profile_id === profileId)` — **absence of an exclusion row means inclusion** (default on). Clicking toggles via `addPageExclusion` / `removePageExclusion`; if the toggled profile is active, `compileAndShow()` runs.
- **Page editing:** Free-form pages open `PageContentEditor.svelte` (full-screen rich text modal). TOC pages open `TocPageEditor.svelte` (settings modal with title text and leader style). Inline rename via pencil icon.
- **TOC page settings:** Stored as JSON in the `content` column: `{ toc_title, leader_style }`. Leader styles: `'dots'` (default), `'dashes'`, `'none'`. The Typst builder reads these via `parse_toc_settings()` and generates the appropriate `#outline(fill: ...)` markup. The ebook builder reads them via `parseTocSettings()` to set the heading text.

**Themes mode:** Still a placeholder. Renders `<p class="shell-placeholder">Theme presets will appear here.</p>`. No preset data, no apply logic. Unchanged.

**Custom mode:** Dropdown selector driven by `filteredCustomTabs`, a `$derived` that filters `ALL_CUSTOM_TABS` by `activeProfile?.target_type`. Tabs marked `targets: ['print']` (i.e. `print-layout` and `header-footer`) are **hidden for ebook profiles**. The full tab list:

```js
const ALL_CUSTOM_TABS = [
  { id: 'chapter-headings', targets: ['print', 'ebook'] },
  { id: 'paragraph',        targets: ['print', 'ebook'] },
  { id: 'headings',         targets: ['print', 'ebook'] },
  { id: 'breaks',           targets: ['print', 'ebook'] },
  { id: 'print-layout',     targets: ['print'] },
  { id: 'typography',       targets: ['print', 'ebook'] },
  { id: 'header-footer',    targets: ['print'] },
  { id: 'trim',             label: 'Target Format', targets: ['print', 'ebook'] },
];
```

Each subpanel receives `profile={activeProfile}` and `onchange={handleCustomSettingChange}`. On change, the parent refreshes `profiles` and recompiles. `TrimSettings.svelte` is the only subpanel that also receives `bind:ebookDevice` — it controls which device frame renders in the ebook preview.

**Export mode:** Shows profile info (name, trim size, page count). For print profiles: "Export PDF" → `handleExportPdf()` → ensures fresh compile → `exportFormatPdf()` (Typst-based; `typst_pdf::pdf(document, &PdfOptions { tagged: false })`) → save dialog → `writeFile`. For ebook profiles: "Export EPUB" → `handleExportEpub()` → `chapterToHtml()` per chapter (via `yDocToProsemirrorJSON` + `generateHTML` with `epubExtensions`) → assembles an `EpubExportRequest` → `exportEpub(request)` → save dialog.

#### Custom subpanels — field reference

All 8 subpanels follow the identical pattern: `$props() = { profile, onchange }`, `$derived.by` parses `profile?.{category}_json` merged with hardcoded `defaults()`, local `$state` mirrors, `scheduleSave()` debounces, `persist()` calls `updateProfileCategory(profile.id, '{category}_json', JSON.stringify(state))`, then `onchange?.()`. Only **full-object** JSON writes — no per-key updates.

| Subpanel | DB column | Debounce | Notable fields (enum values in parens) |
|---|---|---|---|
| `ChapterHeadings.svelte` | `chapter_headings_json` | 250ms | `number_format` (none/numeric/chapter_numeric/word/chapter_word/roman/chapter_roman), `{number,title,subtitle}_{enabled,font,size_pt,align,style,tracking_em}`, `sink_em`, `space_*_em`, `start_on` (any/recto/verso, print-only), `rule_{above,below}` + `rule_thickness_pt`, `image_enabled`, `image_individual`, `image_default` (base64), `image_position` (above_number/between_number_title/between_title_subtitle/below_heading/cover_heading/dedicated_page), `image_width_pct`, `image_align`, `image_light_text` (only when position=cover_heading) |
| `ParagraphSettings.svelte` | `paragraph_json` | 250ms | `drop_cap_{enabled,lines,font,color,quote_mode,fill_pct}` (quote_mode: first_char/both_together/letter_only/disable_on_dialogue), `small_caps_{enabled,words}` (words: 3/5/8/-1), `apply_when` (chapter/breaks/both), `paragraph_style` (indented/spaced/both), `indent_em`, `spacing_em`, `prevent_widows`, `prevent_orphans`, `max_consecutive_hyphens` (2/3/4/99), `last_line_min_chars` (0/3/5/8), `hyphen_aggressiveness` (low/normal/high). **Gotcha:** uses `eval()` in its `set(field, value)` helper — don't copy that pattern. |
| `HeadingsSettings.svelte` | `headings_json` | 250ms | Flat map of `{h2,h3,h4}_{enabled,font,size_pt,align,style,tracking_em,space_above_em,space_below_em,keep_with_next,rule_above,rule_below}` for H2/H3/H4 + global `no_indent_after`. State is a flat object keyed by prefixed strings, not a nested structure. |
| `BreaksSettings.svelte` | `breaks_json` | 250ms | `style` (none/blank/dinkus/asterism/rule/custom/image), `custom_text`, `image_data` (base64), `image_width_pct`, `space_above_em`, `space_below_em`, `keep_with_content` |
| `PrintLayoutSettings.svelte` | `print_layout_json` | 300ms | `margin_{top,bottom,outside,inside}_in` (always inches internally, UI converts mm via `$lib/unitPreference.js`), `justify`, `hyphens`. "Reset to recommended" button calls `getRecommendedMargins(w, h)` from `$lib/marginDefaults.js` and persists immediately. **Hidden for ebook profiles.** |
| `TypographySettings.svelte` | `typography_json` | 200ms | `font`, `size_pt` (9–18), `line_spacing` (1.15/1.25/1.4/1.5/1.75/2.0). **Fallback pattern**: when the JSON column is empty or unparseable, falls back to legacy scalar columns `font_body`, `font_size_pt`, `line_spacing` — preserving older profiles. |
| `HeaderFooterSettings.svelte` | `header_footer_json` | 250ms | Most complex. `slots` map with 12 keys (`{verso,recto}_{header,footer}_{left,center,right}`), each `{content, custom, font, size_pt, style}`. Content enum: none/page_number/book_title/chapter_title/author_name/series_name/book_number/custom. Style: normal/italic/smallcaps/uppercase. Size: 7–12pt. Visual page diagram with clickable slot cells. Globals: `suppress_on_chapter_start`, `suppress_header_on_pages`, `suppress_footer_on_pages`, `header_separator`, `footer_separator`, `separator_thickness_pt`, `margin_{left,right}_in`, `extend_no_header`, `extend_no_footer`. **Hidden for ebook profiles.** |
| `TrimSettings.svelte` | **none (writes to scalar columns)** | immediate | **The exception.** Does NOT call `updateProfileCategory`; calls `updateFormatProfile(...)` directly to update scalar `trim_width_in`, `trim_height_in`, `target_type`. Uses `$lib/trimSizes.js` (`TRIM_CATEGORIES`, `PLATFORMS`, `findSize`, `supportedPlatforms`) for the size catalog. Also exposes the ebook device picker as a `$bindable()` `ebookDevice` prop. **Print mode:** proportional thumbnail + search + category-grouped catalog + custom dimensions. **Ebook mode:** device list (Kindle Paperwhite, Kindle Oasis, iPad variants, iPhone variants, Android, Kobo Libra). **The `trim_json` column exists but is completely inert — never read by `format.rs` or written by any subpanel. Reserved space.** |

#### `PageContentEditor.svelte` — rich text editor for free-form pages

Full-screen modal (`fixed inset: 0, z-index: 2000`) opened via the edit-content button on free-form format pages. Uses TipTap with: `StarterKit.configure({ heading: false, history: true })` (headings disabled — font sizes used instead; history ON, unlike the main editor which uses Yjs undo), `TextStyle`, `FontSize` + `FontFamily` (from `@tiptap/extension-text-style`), a custom `ImageNode`, `TextAlign.configure({ types: ['paragraph'] })`, `Placeholder`. **Paste is plain-text only** (`editorProps.handlePaste` strips all formatting) — users bring raw text in and style it with the toolbar. This prevents CSS font-family stacks and inline styles from leaking into the Typst markup builder.

**Custom image node** is `inline: false, group: 'block', draggable: true` with attributes `src, alt, width` (string like `"300px"`). Manual DOM NodeView: outer centering div + inline-block frame + `<img>` + bottom-right teal resize handle. Handle drag attaches `document` mousemove/mouseup listeners, `newWidth = max(40, startWidth + dx)`, on mouseup dispatches `tr.setNodeMarkup(pos, undefined, { ...attrs, width: finalWidth })` to persist.

**Content format:** stored as ProseMirror JSON stringified (`editor.getJSON() → JSON.stringify`). `parseInitialContent(raw)` handles: empty → blank doc / starts with `{` → JSON.parse / else → split by `\n\n` and wrap paragraphs. The page canvas is sized to exact profile inches at 72dpi (`width: {w}in; height: {h}in; padding: {margins}in`) with a red dashed "Page boundary" marker below — content can overflow visibly so the author sees it. `verticalAlign` state (`'top'` | `'center'` | `'bottom'`) is read from `page.vertical_align`, sent back via `onsave({ content, verticalAlign })`, applied via `data-valign` CSS. Explicit Save button + Ctrl+S; no auto-save. **Images are embedded as base64 data URIs** — large images grow the `.iwe` file directly.

#### `TocPageEditor.svelte` — TOC page settings editor

Small centered modal opened via the gear button on TOC-type format pages. Two settings: **title** (text input, default "Contents") and **leader style** (dots/dashes/none, rendered as clickable option cards with preview text). Includes a mini page preview showing 5 sample entries. Settings stored as JSON in the `content` column: `{ toc_title: string, leader_style: 'dots'|'dashes'|'none' }`. Save + Ctrl+S; Escape cancels.

#### Ebook output — now real, no longer a placeholder

Ebook output previously showed "Ebook preview coming soon". That has been replaced with a live preview + real EPUB export.

**Ebook preview** uses **epub.js** (`npm: epubjs`) for proper ereader-style paginated rendering. When `activeProfile?.target_type === 'ebook'`, `compileAndShow()` calls `renderEbookPreview()` which:

1. Builds a real EPUB via `buildEpubForPreview()` — same pipeline as the export (`exportEpub` Rust command) but with images downscaled on the JS side for speed (`downscaleHtmlImages` → canvas resize to 600px max)
2. Passes the EPUB bytes as an `ArrayBuffer` to `ePub(arrayBuffer)` (epub.js)
3. Renders via `ebookBook.renderTo(element, { width, height, spread: 'none', flow: 'paginated', allowScriptedContent: true })`
4. Generates global page locations via `ebookBook.locations.generate(1024)` for whole-book page numbering
5. Listens to `relocated` events to update `ebookCurrentPage` via `locations.locationFromCfi()`
6. Navigation: `ebookRendition.next()` / `ebookRendition.prev()` handle both within-section pagination and cross-section navigation

Device dimensions come from `DEVICE_DIMS` keyed by `ebookDevice` (default `'kindle-paperwhite'`, 300×480px). The outer device frame adds 24px padding for a bezel look. Below the screen: prev/next buttons + page indicator (e.g. "42 / 165"). `pageCount` is 0 in ebook mode; the timing bar is hidden; the IntersectionObserver is torn down.

**Performance note:** The preview EPUB build is slow (~15-20s) because images are embedded as base64 in HTML, sent over IPC as JSON, regex-scanned and decoded in Rust, then zipped. The `downscaleHtmlImages` helper mitigates this by shrinking large images to 600px on the JS side before sending. The proper fix is migrating to BLOB storage (see Work-in-progress section).

**EPUB export** — `src-tauri/src/epub.rs` is fully implemented and registered. Single command `export_epub(request: EpubExportRequest) -> Result<Vec<u8>, String>`, **stateless** (takes no `AppState` — all content comes from the frontend payload). Uses `epub-builder = "0.8"` with `ZipLibrary::new()` at EPUB 3.0. Request shape:

```rust
struct EpubExportRequest {
    title, author, language, description: String,
    chapters: Vec<EpubChapter>,       // { title, subtitle, html }
    front_pages: Vec<EpubPage>,       // { title, role (from ebook_metadata_tag), html, position }
    back_pages: Vec<EpubPage>,
    cover_image: Option<String>,      // base64 data URL, decoded via hand-rolled base64 decoder
    css: String,                      // written as epub stylesheet.css
}
```

Flow: set metadata → decode cover image + `add_cover_image` → `stylesheet(css)` → iterate front pages (`build_page_html` + `wrap_xhtml`, TOC pages use `ReferenceType::Toc`, title pages `TitlePage`, else `Text`) → iterate chapters (`build_chapter_html`, wrapped in `<section epub:type="chapter">`) → iterate back pages → `builder.generate(&mut output)` → return bytes. `epub_type_for_role(role)` maps ebook metadata tags to EPUB semantic types (e.g. `acknowledgments → acknowledgments`, unknown → `bodymatter`). The frontend passes `ebook_metadata_tag || page_role` as the `role` field.

**Known EPUB gaps:**
- `EpubChapter.subtitle` is in the struct but no UI populates it.
- `EpubPage.position` is carried but unused in the epub builder (front/back is encoded by which Vec the page is in).
- The CSS source is profile-dependent — the frontend decides what to send.

#### Typst markup generation — `build_typst_markup` (format.rs:1017-1599)

Signature: `fn build_typst_markup(profile, front_pages, chapters, back_pages, book_title, author_name, series_name, series_number) -> (String, Vec<String>, ImageMap)`. Chapters param is `&[(i64 chapter_id, String title, String subtitle, String chapter_image_data, String text)]`.

Generated Typst document structure:

1. **Metadata variables** (top): `#let iwe-book-title = [...]`, `#let iwe-author-name = [...]`, `#let iwe-series-name = [...]`, `#let iwe-series-number = [...]`. Referenced by header/footer slot expressions.
2. **Page setup**: `#set page(width: Xin, height: Xin, margin: (top:..., bottom:..., outside:..., inside:...))`. Margins read from `print_layout_json` with fallback to scalar columns. If `extend_no_header`/`extend_no_footer` is set and no header/footer slot has content, top/bottom margin is reduced to `(margin * 0.6).max(0.375)`.
3. **Header/footer**: `#set page(header: context { ... })` — uses a `context {}` code block (not markup mode) with `calc.even(here().page())` for recto/verso branching, and a `grid(columns: (1fr, 1fr, 1fr), ...)` for three-column alignment. Separator lines use `line(length: 100%, stroke: Npt)`. Chapter-start suppression queries all H1 heading locations and hides header/footer on pages containing one. Both header and footer are independently suppressible on front/back matter pages via `suppress_header_on_pages` / `suppress_footer_on_pages` toggles. When `numbering` is active but no custom footer slots exist, `footer: none` is emitted to suppress Typst's built-in page number.
4. **Typography**: `#set par(first-line-indent: Nem, spacing: Nem)` and `#set text(costs: (widow: X, orphan: Y))`. Body font/size/leading are scoped per-chapter in `#[...]` blocks, not at document level.
5. **Heading show rules**: **H1 is suppressed entirely** with `#show heading.where(level: 1): it => {}` — chapter titles are rendered manually via the chapter-heading block. H2/H3/H4 get show rules built from `headings_json`. The dropcap preamble `#import "@preview/droplet:0.3.1": dropcap` is prepended only when drop caps are enabled.
6. **Front matter**: each `FormatPage` in position=front gets an anchor `#[#metadata(none) <iwe-fp-{id}>]`, then `build_front_matter_page()`, then `#pagebreak()`. Roman numeral page numbering if any slot uses `page_number`.
7. **Chapter loop**: `#pagebreak()` or `#pagebreak(to: "odd")` → anchor `<iwe-ch-{chapter_id}>` → `#heading(level: 1, outlined: true)[title]` (invisible via show rule but indexed for TOC/outline) → sink `#v(Nem)` → optional rules + number + title + subtitle → chapter image at configured position → body in `#[` block with scoped `#set text` + `#set par` → paragraph loop: scene-break sentinels trigger `emit_scene_break()`, opener paragraphs get `emit_opener_paragraph()` (drop cap + small caps), subsequent paragraphs after breaks/chapter start get `#par(first-line-indent: 0em)[...]`.
8. **Back matter**: same as front matter but after a `#pagebreak()`.

**Image ingestion**: `ingest_image(src, images)` decodes data URLs, assigns virtual path `/iwe-img-{fnv1a_hash}.{ext}` (dedupes identical images by hash), stores bytes in `ImageMap`. `IweWorld.file()` serves these when Typst requests them.

**Scene break detection**: `extract_chapter_blocks_from_bytes` detects `horizontalRule` elements in the Y.Doc and emits `ChapterBlock::SceneBreak` directly — no sentinel strings needed. The old `is_scene_break_sentinel()` function and `extract_text_for_format_from_bytes` are still in the code but unused (dead code from the pre-formatting pipeline).

#### The `IweWorld` — Typst `World` trait implementation (format.rs:60-150)

```rust
struct IweWorld {
    library: LazyHash<Library>,
    book: Arc<LazyHash<FontBook>>,     // shared with FormatState.font_cache
    slots: Arc<Vec<FontSlot>>,          // system fonts via typst-kit
    embedded: Vec<Font>,                // the 4 Liberation Serif variants
    source: Source,                     // single detached source, the markup string
    images: HashMap<String, Vec<u8>>,   // keyed by /iwe-img-{hash}.{ext}
    packages: Arc<PackageStorage>,      // shared package cache
}
```

Key points:
- **Single in-memory source.** `Source::detached(markup)` — no on-disk path. `main()` returns `self.source.id()`.
- **File resolution order** in `file(id)`: check `self.images` first (for `/iwe-img-*` paths), then try package resolution via `PackageStorage::prepare_package()`, else `FileError::NotFound`.
- **Font resolution**: `font(index)` tries `self.slots[index]` (system fonts), falls back to `self.embedded[index - slots.len()]`. Embedded fonts are appended to the font index space.
- `today()` returns `None` — Typst date features silently fail.
- **A new `IweWorld` is created per compile.** Only the `FontCache` is reused via `Arc`.

#### `FormatState` (format.rs:1650-1683) — Tauri managed state

```rust
pub struct FormatState {
    document: Mutex<Option<PagedDocument>>,  // last compiled doc
    font_cache: Mutex<Option<Arc<FontCache>>>, // lazy init on first compile
    packages: Arc<PackageStorage>,           // $TEMP/iwe-typst-packages
}
```

Built in `run()` via `.manage(format::FormatState::new())`. The stub version (non-format builds) is just `Mutex<()>`. The `PackageStorage` uses `std::env::temp_dir().join("iwe-typst-packages")` with `Downloader::new("iwe/0.1.0")`. **Typst package downloads happen at first compile with network access** — machines offline during a first drop-cap compile will fail on `@preview/droplet:0.3.1`. Cache lives in the OS temp dir, not app data dir, so it can be cleared by OS cleanup.

#### Fonts: `list_system_fonts` (format.rs:1851-1864)

Calls `FormatState::get_or_init_fonts()` (which lazy-inits via `typst_kit::fonts::FontSearcher::new().include_system_fonts(true).search()`), collects families from `cache.book.families()`, sorts and dedupes. **Calling `list_system_fonts` before the first compile initializes the font cache** — this is deliberate so `FontPicker.svelte` can trigger discovery without needing a compile first. The 4 Liberation Serif embedded variants (`include_bytes!` at format.rs:22-25) are always available as a fallback tail in the font index space.

#### Three PDF paths + EPUB path — crucial to keep straight

- `/print/+page.svelte` (popup): Y.Doc → PM JSON → HTML via `yDocToProsemirrorJSON` + `generateHTML`, applies A4/Book CSS, `window.print()`. Fast, rough, no layout control.
- Toolbar `Export → PDF`: `genpdf` crate in Rust (`lib.rs:686-823`) with embedded Liberation Serif, A4 or 5×8 preset. Not Typst.
- Format editor print profiles: `format::export_format_pdf` → `typst_pdf::pdf(document, &PdfOptions { tagged: false })` on the cached `PagedDocument`. **Publication quality.**
- Format editor ebook profiles: `epub::export_epub` → `epub-builder` crate, stateless, frontend pre-renders all HTML. **Not Typst at all** — EPUBs are reflowable HTML, Typst produces paged documents.

#### Known gaps and gotchas (READ BEFORE EDITING)

1. **`duplicate_format_profile` does not copy JSON category columns** (db.rs:2086-2089). The INSERT SELECT lists only scalar columns (`target_type`, trim, margins, font, size, leading). The duplicate starts with `'{}'` for all 8 JSON columns. Duplicating a heavily customized profile silently loses all Custom mode settings. **Fix candidate.**
2. **`trim_json` is inert.** The column exists, is in `FORMAT_CATEGORY_COLUMNS`, but `TrimSettings.svelte` calls `updateFormatProfile` directly (not `updateProfileCategory`) and `format.rs` never reads `trim_json`. Reserved for future use.
3. **Dual margin representation.** Margins live in both scalar columns AND `print_layout_json`. Typst prefers `print_layout_json` with fallback to scalars. `update_format_profile` updates only scalars; `PrintLayoutSettings` writes only JSON. On older profiles, both may be populated with different values — JSON wins.
4. **No SQL CHECK constraints on `target_type`, `position`, `include_in`, `vertical_align`, `page_role`.** All enforcement is UI-side. Any string passes the DB layer.
5. **`list_page_exclusions` returns the entire table.** Filtering by profile is client-side. There's also `list_excluded_page_ids_for_profile` used only internally by `compile_preview`.
6. **Images stored inline as base64** in `format_pages.content` AND in `chapter_headings_json.image_default` AND in `breaks_json.image_data`. Large images grow the `.iwe` file significantly.
7. **`compile_preview` holds the DB mutex through the entire compile** (4 phases, potentially several seconds). Blocks all other DB operations. Architectural issue if we ever want concurrent DB reads.
8. **No compile debounce.** Rapid setting changes trigger overlapping compiles. The `rendering` flag shows a spinner but doesn't cancel or queue.
9. **Chapter/page label introspector lookups can silently miss.** If a chapter anchor fails to resolve (e.g. empty chapter text), `section_pages` just won't have an entry — no error. Scroll-to-section will no-op.
10. **Custom page titles are never rendered.** The `title` field is an internal label for the Pages sidebar. Neither ebook nor print outputs it. The Typst empty-page fallbacks were removed (except TOC `#outline`). The ebook `<h1>` title injection was also removed. If a user wants a heading on a custom page, they add it in PageContentEditor.
11. **`escape_typst` handles Typst list markers.** Leading `- `, `+ `, `/ `, `– `, `— ` (even with preceding whitespace) are escaped with `\` to prevent content blocks from being parsed as lists. Bold uses `#strong[...]` and italic uses `#emph[...]` (not `*...*` / `_..._` shorthand) to avoid collision with Typst list/emphasis syntax at line boundaries.
12. **`apply_pm_marks` handles CSS font-family stacks.** TipTap's `fontFamily` values (e.g. `"Times New Roman", serif`) are parsed by `extract_font_name()` to extract just the primary font name. Empty `fontSize` strings are skipped to avoid generating invalid `size: ` in Typst markup.
13. **Bundle resources are explicit, not wildcard.** `tauri.conf.json` lists `resources/models/model.onnx` and `resources/models/tokenizer.json` individually (not `resources/models/*`) to prevent `.bak` files from bloating the installer.

#### EPUB export pipeline (how preview and export stay in sync)

The ebook preview (`buildEpubForPreview` → epub.js) and the EPUB export (`handleExportEpub`) share the same code path for content rendering, CSS generation, and image handling — both call the Rust `exportEpub` command to build real EPUB bytes. The old `generateEbookPreview` function that built HTML directly is still in the file but no longer called (dead code). Any time you change the export pipeline, the preview automatically picks up the changes.

**Shared helpers** (all in `format/+page.svelte`):
- `resolveEbookSettings(profile)` — parses the five Custom category JSONs (`chapter_headings_json`, `paragraph_json`, `headings_json`, `breaks_json`, `typography_json`) merged with hand-kept `*_DEFAULTS` constants. Typography falls back to legacy scalar columns (`font_body`, `font_size_pt`, `line_spacing`) when the JSON is empty.
- `buildEbookCss(settings, { inline })` — returns the full ebook CSS string. `inline: true` wraps in a `<style>` tag and adds preview-only `.ebook-chapter-break` scaffolding (for the preview). `inline: false` returns raw CSS (for the exported `stylesheet.css`).
- `buildParagraphCss` / `buildHeadingsCss` / `buildBreaksCss` — per-category CSS builders. All output is prefixed with `.ebook-body` so a single class scopes everything.
- `renderChapterHeadingHtml(chapter, index, chHeadings)` — generates the chapter heading block (number / title / subtitle / rules / image) honoring all `chapter_headings_json` settings. Used by both preview and export so they produce identical heading markup.
- `substituteImageBreaks(html, breaks)` — post-processes HTML to replace every `<hr>` with a `<div class="scene-break-img"><img/></div>` wrapper when `breaks.style === 'image'`. Done via HTML substitution rather than CSS `::after` because replaced-content pseudo-elements size unpredictably across arbitrary image aspect ratios.
- `htmlToXhtml(html)` — self-closes HTML void elements (`<hr>` → `<hr/>`, `<img ...>` → `<img .../>`, plus the other 12 void tags). EPUB is strict XHTML — browsers tolerate the HTML form, epubcheck does not. **Only applied on the export path** — the in-app preview uses plain HTML because browsers parse it fine.
- `chapterToHtml(chapter)` — Y.Doc → PM JSON → HTML via `generateHTML(json, chapterHtmlExtensions)`. The extension list MUST include `NoteMarker`, `StateMarker`, `TimeBreak` custom nodes or chapters that contain them silently render as empty (the catch swallowed the schema error). See import at top of file.
- `pageToHtml(page)` — parses the format_pages row's PM JSON (from PageContentEditor) and runs `generateHTML(parsed, pageHtmlExtensions)`. A minimal inline `ImageNodeForHtml` handles the image node with width baked into the style attribute (no NodeView available in HTML generation).

**Preview pipeline** (`buildEpubForPreview` → epub.js):
1. Resolve settings, build raw CSS (same as export path)
2. Convert chapters to XHTML via `chapterToHtml` + `htmlToXhtml`, downscale large images via `downscaleHtmlImages`
3. Build front/back matter pages (filtered by profile exclusions), downscale images
4. Call `exportEpub` Rust command with the request — produces real EPUB bytes
5. Feed `ArrayBuffer` to `ePub()` from epub.js, render with `rendition.display()`
6. epub.js handles all pagination via CSS columns internally (the same approach used by Readium and other production ebook readers)

**Export pipeline** (`handleExportEpub`):
1. Resolve settings, build raw CSS (no `<style>` wrapper, no preview-only scaffolding)
2. For each chapter: `renderChapterHeadingHtml(ch, i) + chapterToHtml(ch)` → `substituteImageBreaks` → `htmlToXhtml` → chapter body
3. For each front/back page: `pageToHtml(p)` → `substituteImageBreaks` → `htmlToXhtml` → page body
4. Send to Rust `export_epub` with `{ chapters, front_pages, back_pages, css, title, author, language, description }`
5. Rust `extract_inline_images` scans every chapter/page HTML for `<img src="data:...">`, decodes each unique image via FNV-1a-hashed filename, stores as separate `OEBPS/images/img-{hash}.{ext}` zip entries, rewrites the HTML `src` to relative paths. Image deduplication is automatic across all chapters/pages
6. Rust `build_chapter_html` wraps each chapter body in `<section epub:type="chapter">` — does NOT auto-generate a `<h1>{title}</h1>` (the JS side already provided the styled heading via `renderChapterHeadingHtml`)
7. Rust `wrap_xhtml` wraps body content in `<div class="ebook-body">` inside `<body>` so the exported XHTML matches the preview's class structure. Same CSS selectors work for both
8. Rust `fix_opf_ids` post-processes the generated zip to strip the buggy `id="epub-creator-0"` from `<dc:language>` (epub-builder 0.8 emits it with a hardcoded wrong id in its OPF template)
9. Frontend calls `validateEpubBytes(arr)` on the returned bytes — runs the lightweight Rust validator and surfaces any issues via toasts before showing the save dialog
10. Save dialog → write file to disk

**Rust-side epub.rs invariants**:
- `EpubExportRequest` has NO `cover_image` field — Rust reads the cover BLOB directly from `book_cover` in the project DB
- `build_chapter_html` takes a pre-rewritten body (with data URLs already extracted). The `chapter` parameter is kept for future use but currently only the index is used
- `wrap_xhtml` takes a `css` parameter for historical reasons but doesn't use it — CSS is attached via the separate `stylesheet.css` file. The param is kept for API stability
- Metadata call order is `title → lang → author` (not author → lang). Swapping this re-introduces the duplicate-id bug because epub-builder's `lang` inherits the previous creator's id if set after `author`
- All inline images get extracted into `OEBPS/images/` BEFORE any `add_content` call, so they appear in the manifest ahead of the chapters that reference them

#### EPUB validation and cross-device testing

**Built-in Rust validator** (`src-tauri/src/epub_validate.rs`) runs automatically after every EPUB export via the `validate_epub_bytes` Tauri command (called from `handleExportEpub`). Uses only `quick-xml` and `zip` — no external dependencies. Catches:
1. Any `.xhtml` / `.html` file in the zip failing to parse as well-formed XML (catches `<hr>`/`<img>`/`<br>` not self-closed, malformed attributes, unclosed tags)
2. `content.opf` not well-formed
3. Duplicate XML `id` attributes in `content.opf` (catches the epub-builder creator/language collision if it ever regresses)
4. Manifest `<item href>` values that don't resolve to files in the zip
5. Spine `<itemref idref>` values that don't resolve to manifest item ids

Issues are returned as `Vec<EpubIssue>` with `{ level, code, file, message }`. The frontend shows an error toast for any `level: "error"` entries and logs the full list to the DevTools console. **Not a full epubcheck replacement** — doesn't validate EPUB3 conformance rules, OCF container structure, accessibility, media-type correctness, or NCX format. It's a regression net for the specific bugs that have actually shipped.

**Official epubcheck** (the W3C/IDPF standard validator) is Java-only. For ad-hoc validation during development, use the Python wrapper (bundles the JAR):

```bash
# One-time install (user site to avoid permission issues on Windows):
pip install --user epubcheck

# Requires Java to be in PATH. On Windows:
where java
# Should return something like C:\Program Files\Eclipse Adoptium\jdk-25.0.1.8-hotspot\bin\java.exe
```

**Run epubcheck on a file**:
```bash
python -c "
from epubcheck import EpubCheck
r = EpubCheck(r'C:\path\to\book.epub')
print('VALID:', r.valid)
print(f'{len(r.messages)} messages')
for m in r.messages:
    print(f'[{m.level}] {m.id} @ {m.location}')
    print(f'  {m.message[:250]}')
"
```

Severity levels: `FATAL` (file won't open), `ERROR` (malformed, some readers may tolerate), `WARNING` (stylistically wrong but opens), `USAGE` (conformance nitpicks). Common error codes:
- `RSC-005` — general parse / structural error (also covers duplicate ids)
- `RSC-016` — fatal XHTML/XML parse error (void elements not self-closed)
- `OPF-0xx` — OPF manifest/spine issues
- `HTM-0xx` — XHTML content issues

**Inspecting EPUB contents without a reader**: EPUBs are ZIP archives. Use Python to peek inside:
```bash
python -c "
import zipfile
with zipfile.ZipFile(r'C:\path\to\book.epub', 'r') as z:
    for info in z.infolist():
        flag = '[STORE]' if info.compress_type == zipfile.ZIP_STORED else '[DEFL] '
        print(f'  {flag} {info.file_size:>10} {info.filename}')
    # Print a specific file:
    print(z.read('OEBPS/content.opf').decode('utf-8'))
"
```

Key files in every EPUB:
- `mimetype` — must be STORED (uncompressed), must be the first entry, contains literal `application/epub+zip`
- `META-INF/container.xml` — points at the OPF file
- `OEBPS/content.opf` — manifest (all files) + spine (reading order) + metadata (title/author/language/etc.)
- `OEBPS/toc.ncx` — legacy EPUB2 TOC (still used by some readers)
- `OEBPS/nav.xhtml` — EPUB3 navigation document
- `OEBPS/stylesheet.css` — CSS (when using `epub-builder`'s `stylesheet()` method)
- Chapter and page XHTML files

**Cross-device testing — epubcheck passing is necessary but not sufficient.** Validation confirms the file is structurally valid EPUB3; it doesn't confirm it *renders correctly* in real readers, which varies wildly due to reader-specific CSS subsets and font handling. Known quirks:
- **Kindle** (KF8/AZW3): strict CSS subset, drop-caps via `::first-letter` often broken, `background-image` on divs patchy, flexbox unreliable, many modern font features stripped
- **Apple Books**: generally spec-compliant but picky about embedded font licensing and has its own font-fallback quirks
- **Kobo**: based on Adobe RMSDK, mostly-compliant but differs on line-height and widow-control
- **Adobe Digital Editions**: conservative baseline most readers emulate, limited flexbox
- **Thorium** (W3C reference): most permissive, renders closest to a browser

**Testing tools to install before releasing a book**:
1. **Kindle Previewer 3** (free, Amazon) — tests all Kindle device models. Biggest market, strictest renderer. Install first.
2. **Apple Books** (macOS/iPadOS native) — second biggest market
3. **Calibre** (free, cross-platform) — tests generic ADE-compatible rendering
4. **Thorium Reader** (free, cross-platform) — W3C reference implementation

Typical iteration: export → check in all four → find quirks → adjust CSS / layout → re-export. This is inherent to the format and there's no way to automate it away.

### Manuscript import — `src/routes/import/+page.svelte` + `src-tauri/src/import.rs`
Popup window. Launched from the Home page's "Import…" button. Supported formats: `.docx` and `.epub`, detected by extension. The Rust side parses into `ImportBlock`s (text, style, `page_break_before`, `is_heading`) and runs a `detect_breaks` pass.

**Five detection methods:** `auto`, `heading`, `page_break`, `pattern`, `blank_lines`. **Auto** tries them in order (heading → page_break → pattern → blank_lines) and picks the first one producing 2–500 breaks. The `pattern` regex matches `(?i)^\s*(chapter|prologue|epilogue|part|book)\b|^\s*[ivxlcdm]+\s*$|^\s*\d{1,3}\s*$` — so Roman numerals and bare numbers count. `blank_lines` treats 3+ consecutive empty paragraphs as a break (Vellum-style).

**UI:** live preview of the full parsed document. Editable project title, format badge (DOCX/EPUB), "Detect by" dropdown to switch methods, chapter count, **Ctrl+scroll zoom** (15–160%), chapter break dividers with editable title and source badge + × button, "Insert break here" buttons in gaps. Yellow validation banner lists chapters <200 words ("short"), >15k words ("long"), and untitled chapters — clickable to scroll to the problem. Import progress overlay: "{n} of {total} chapters". On success emits a `projects-changed` event so the home page refreshes.

### Comparative Works & dev-only batch analyse
`famous_books.rs` owns a separate `famous_books.db` (app data dir) storing `{title, author, source, word_count, analyses_json}` rows. The JSON blob holds the results of all 6 analyses (chapter, pacing, readability, paragraphs, adverbs, word_frequency) so charts can overlay a "famous book" benchmark.

Populated via `src/routes/analyse/+page.svelte`, a **dev-only** popup launched from the ⚗ button on each project row on the home page (visible only when `import.meta.env.DEV`). Runs all 6 analyses sequentially, shows a summary grid + collapsible raw JSON, and has a "Save to library" button that calls `save_library_book`.

Consumed by **AnalysisPanel.svelte**'s "Comparative Works" tab (stored per-project via `project_settings.comparative_book_id`). When a book is selected, the Word Frequency, Adverb Density, Chapter Analysis, Pacing, Readability, and Paragraph analyses all overlay comparison data in orange.

### Automatic backups — `lib.rs:21-174`
Triggered inside `update_chapter_content` whenever `(now - last_backup_at) >= backup_interval_minutes` since the last backup and `interval > 0`. Runs on a background `std::thread::spawn` so it doesn't block typing. Copies the `.iwe` file to `{projects_dir}/backups/{book_name}/{book_name}-YYYY-MM-DD-HH-MM-SS.iwe`. **Strips semantic tables from the backup** (`DELETE FROM semantic_embeddings`, `DELETE FROM semantic_index_status`, `VACUUM`) to keep backups small. **Cleanup:** keeps everything within 7 days, then prunes to one backup per day for older dates.

Interval is stored in `writing_settings.backup_interval_minutes` (default 60, 0 disables) and also mirrored to `settings.json` so the home page can display it.

### Relationship Search — modes that actually exist
`SearchPanel.svelte` exposes only **two** relationship search types: `near` and `without` (see `scanner.rs:606-747`). The distance slider ranges from 100 to 5000 **characters** (not words). "Near" returns a lead-in + collapsible middle + lead-out text slab with both entities highlighted; "without" returns ~300-char context around every A mention that has no B mention within `max_distance`. Do not add "same chapter" / "appears before" modes casually — they would require a new scanner path.

### Search — 5 modes
Both **Text Search** and **Dialogue Search** expose the same 5 mode pills: `Standard`, `Aa` (case-sensitive), `[w] Word` (whole word), `.* Regex`, `~ Fuzzy`. The mode is passed as a set of bools to the Rust command (`case_sensitive`, `whole_word`, `use_regex`, `fuzzy`). Dialogue Search routes the query through `text_utils::extract_dialogue` first so only dialogue spans are searched.

### Typewriter mode
Toggle in Home → Settings, persisted to `settings.json`. When active, `Editor.svelte::typewriterScroll` runs on every selection change with a 16ms debounce: computes cursor Y position via `view.coordsAtPos(from)`, calculates the offset from the `.editor-scroll` container's vertical center, and smooth-scrolls the container if the offset is > 10px. Only the scroll position changes — no document mutation.

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
All charts across the app (chapter analysis, heatmap, pacing waveforms, readability, paragraph length) use raw `canvas.getContext('2d')` calls. No chart library. The design system uses: `#faf8f5` background, `#2d6a5e` teal for primary data, `#d97706` amber for secondary, `#6b6560` for labels, Libre Baskerville for titles, Source Sans 3 for UI text.

### Readability Score (Flesch-Kincaid)
Popup window (`/readability`) computing Flesch-Kincaid grade level per chapter and manuscript-wide. Uses `text_utils::extract_sentences()` for sentence splitting and a compiled `phf` hash map (`syllable_data.rs`) for syllable counting with a vowel-group heuristic fallback.

**Syllable data pipeline:** `src-tauri/resources/syllables-list.csv` (7,596 words) → `generate_syllables.py` → `src/syllable_data.rs` (compile-time `phf::Map`). The CSV is dev-only and never ships in the binary. The `phf` crate compiles the map into the binary as hashed bucket structures — not human-readable strings. Run `python generate_syllables.py` from `src-tauri/` after updating the CSV.

**Sentence counting note:** Our sentence splitter is more accurate than most online tools — it correctly handles abbreviations (Mr., Dr., U.S.A.), ellipsis, initials, and dialogue attribution. This means we count fewer sentences than naive tools, producing slightly higher grade levels. This is intentional and more correct.

### Paragraph Length Variation
Popup window (`/paragraphs`) showing paragraph word counts as vertical columns per chapter. Uses `ydoc::extract_text_with_breaks()` which inserts `\n\n` between Y.Doc top-level blocks. Flags runs of 3+ consecutive paragraphs within 15% of each other's word count as "monotonous" (shown in amber). Computes std dev and variation % per chapter.

### Writing stats tracking
On every content save (500ms debounce), the project page computes word count delta and calls `logWritingActivity()`. The Rust side updates `daily_stats` atomically. Active time is computed from gaps between consecutive activities (capped at session_gap_minutes).

### Settings storage
App-level settings (projects folder, panel widths, `uiScale`, `formatSidebarWidth`, `typewriterMode`, `semanticIndexDelay`, `backupInterval`, `formatLengthUnit`) are stored in `settings.json` in Tauri's app data dir via `tauri-plugin-fs`. Project-level settings (writing goals, entity data) are in the `.iwe` SQLite file.

## Common Patterns

### Shared text utilities (`text_utils.rs`) — CRITICAL

All text analysis that needs dialogue or sentence boundaries MUST use the shared functions in `text_utils.rs`. Do not implement your own splitting logic.

**`extract_dialogue(plain) → Vec<DialogueSpan>`** — Finds all dialogue spans in plain text. Handles straight quotes, curly quotes, guillemets, CJK brackets, and mixed quote styles. Uses heuristics to disambiguate straight `"` as opener vs closer (checks if next char is a letter). Caps at 500 chars per span to prevent runaway mismatched quotes. Used by: dialogue search, chapter analysis (dialogue/narrative split), adverb analysis.

**`extract_sentences(plain) → Vec<Sentence>`** — Robust sentence extraction. Handles abbreviations (Mr. Mrs. Dr. etc.), ellipsis, decimal numbers, initials (J.K.), multiple punctuation (!! ??), closing quotes after punctuation. Returns text, char_start, char_end, word_count per sentence. Used by: chapter analysis, pacing analysis, similar phrasing, heatmap.

**`count_words(text) → usize`** — Word counting utility.

### Analysis tools (`analysis.rs`)

All analysis commands live in `analysis.rs`, separate from the core entity scanner. This keeps the scanner focused on Aho-Corasick matching. The AnalysisPanel selector groups tools as **Repetition / Style / Overview / Comparison**.

**Inline tools** (results in the right panel, clickable to navigate):
- `word_frequency` — word repetition counts with per-chapter breakdown; also drives the **Cluster Finder** (same command with a window_size parameter for co-occurring clusters)
- `find_similar_phrases` — sentence similarity detection via Jaccard + LCS
- `adverb_analysis` — finds adverbs in dialogue attribution near speech verbs; detects "redundant" cases like `whispered quietly` via `wordlists.rs`; reports a "tag adverb rate" = attributions_with_adverbs / total_dialogue_spans

**Inline toggle (no Rust command):**
- **Dialogue Detection** — a JS-side toggle that walks `buildTextMap` text and emits `debugDecoKey` decorations over dialogue spans in the editor. Runs entirely in the main window; no popup. Implemented in the project page's `computeDialogueRanges` function, not `analysis.rs`.

**Popup window tools** (each launches a `WebviewWindow`; all use `ongotochapter`-style cross-window navigation):
- `chapter_analysis` → `/chapters` — 4 canvas charts + 12-column breakdown table, with optional Comparative Works overlay
- `generate_heatmap` → `/heatmap` — two canvas views: chapter grid AND sentence timeline across the whole manuscript
- `pacing_analysis` → `/pacing` — whole-book sentence-length waveform with smoothing/zoom sliders and optional comparison overlay
- `readability_analysis` → `/readability` — Flesch-Kincaid overview + per-chapter sentence-level canvases with grade-zone color bands
- `paragraph_length_analysis` → `/paragraphs` — column chart per chapter, flags monotonous runs (3+ paragraphs within 15% of each other)

**Comparison group:**
- `comparative` — a picker (no Rust command) that reads `famous_books.db` via `list_library_books` and writes `project_settings.comparative_book_id`. When set, the other tools overlay benchmark data.

**To add a new analysis tool:** add an entry to the `analysisTools` array in `AnalysisPanel.svelte` (in one of the four groups), add state variables, add the `{:else if subTab === 'mytool'}` view, and if it needs Rust processing add the command to `analysis.rs` + register it in `lib.rs::invoke_handler`.

### Adding a new Tauri command

1. Add the function to the appropriate Rust file: `db.rs` (data), `scanner.rs` (entity scanning), `analysis.rs` (analysis tools), `text_utils.rs` (shared text utilities), `ydoc.rs` (Y.Doc operations)
2. Add the `#[tauri::command]` wrapper in `src-tauri/src/lib.rs`
3. Register it in the `invoke_handler![]` macro in `lib.rs`
4. Add the JS wrapper in `src/lib/db.js`
5. Call it from Svelte components via `import { myFunction } from '$lib/db.js'`

### Adding a new popup window (separate Tauri window)

Use this for analysis/import/print-style views that need their own window chrome and can read the DB independently.

1. Create `src/routes/mywindow/+page.js` (with `prerender = false, ssr = false`) and `+page.svelte`
2. Add `"mywindow*"` to the `windows` array in `src-tauri/capabilities/default.json`
3. Launch with `new WebviewWindow('mywindow-' + Date.now(), { url: '/mywindow', title: '...', width, height, resizable: true })`
4. Add `:global(html), :global(body) { overflow: auto !important; height: auto !important; }` to enable scrolling
5. If the window reads chapter content, it can either call Rust commands directly (they'll use the `AppState.db` the main process holds) or convert Y.Doc bytes to HTML via `yDocToProsemirrorJSON` + `generateHTML` using the same extension set as `+layout.svelte` (StarterKit without history + TextAlign + Superscript + Subscript)
6. For click-to-jump features, call `emitTo('main', 'navigate-to-position', { chapterId, searchText, charPosition })` — the project layout forwards to the editor page's `handleGoToChapter`

### Adding a new project subroute (inline nav tab)

Use this for views that belong in the editor shell and should share the open project context (Kanban, Stats, Time Flow, Palettes, Format).

1. Create `src/routes/project/[filename]/mythingy/+page.js` + `+page.svelte`
2. Add a `<a href="{basePath}/mythingy" class="nav-tab">` entry in `src/routes/project/[filename]/+layout.svelte` and extend the `activeSection` derivation so the tab highlights correctly
3. No capabilities change needed (it's the same window as the editor)
4. Call the same `db.js` wrappers the editor page uses — the project is already open

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

## Work-in-progress — features NOT yet built

This list is authoritative. If you're an agent working on this codebase, do not assume any of the following exist just because they appear in `spec.md`, older CLAUDE.md versions, or online writing-tool discussions. They are scoped but unimplemented.

**UI surfaces that are placeholder or shell-only:**
- **Format editor → Themes mode** — the sidebar tab exists and renders "Theme presets will appear here." No preset data or apply logic.
- **Entity custom fields** — the `entity_fields` table exists in the schema but has zero UI. Don't delete the table; it's reserved.
- **`trim_json` column on `format_profiles`** — declared, migrated, and in `FORMAT_CATEGORY_COLUMNS`, but completely inert. `TrimSettings.svelte` writes to scalar columns directly via `updateFormatProfile`, and `format.rs` never reads `trim_json`. Reserved space for future trim-specific metadata (bleed, safe zones, etc.).

**Format editor — partially built but with known gaps (NOT fully "not built"):**
- **Ebook preview uses epub.js** for real paginated rendering. The preview generates a full EPUB and feeds it to epub.js, which handles pagination identically to production ebook readers. What's still weak: `EpubChapter.subtitle` has no UI that populates it, `EpubPage.position` is carried but unused by the builder, and the CSS sent to Rust is generated ad-hoc from `typography_json` — there's no per-profile ebook stylesheet editor yet.
- **Image storage is base64-in-HTML (slow).** All images (map pages, chapter images, custom page images) are stored as base64 data URLs embedded in HTML content strings. This causes the EPUB preview build to take ~15-20s because images must be regex-scanned from HTML, decoded from base64, and re-encoded into the zip. **The proper fix:** migrate to a `content_images` table with BLOB storage, reference images by ID in HTML (e.g. `<img src="iwe-image://123">`), and have Rust read BLOBs directly from the DB at export time — same pattern as the book cover. Expected speedup: 20s → under 2s. Affects: `format_pages.content` (PageContentEditor images), `chapter_headings_json.image_default`, `breaks_json.image_data`, map page `image_data`, and any chapter content with pasted images.
- **`duplicate_format_profile` does not copy JSON category columns.** The INSERT SELECT at db.rs:2086-2089 copies only scalar fields. Duplicating a heavily customized profile silently resets all 8 Custom-mode categories to `'{}'`. Scoped to fix but not done.
- **No compile debounce.** Rapid setting changes trigger overlapping Typst compiles. Harmless but wasteful.

**Features from `spec.md` that are NOT in the code:**
- Minimap (thin scrollbar preview with entity dots)
- Breadcrumbs ("Part > Chapter > Scene" location display)
- Quick Open (Ctrl+P fuzzy across chapters/entities/text)
- Peek popup (inline card summary on hover/alt-click)
- Autocomplete for entity names as you type
- Split editor
- Bookmarks panel
- Find and Replace
- Rename symbol (rename entity → rewrite all occurrences in the manuscript text)
- Distraction-free mode (typewriter mode is the closest thing currently)
- Problems panel / orphan detection / unused symbol warnings

**Features that exist in the backend but have no UI entry point:**
- Chapter drag-reorder. `reorder_chapters` command works and is called from the Kanban "Chapters" board (which drags chapter columns), but `ChapterNav.svelte` itself has no drag handling — only rename/delete/restore.
- Word counts per chapter in the chapter list sidebar. `get_all_chapter_word_counts` runs on project load and populates `wordCounts` state in the project page, but `ChapterNav.svelte` doesn't render them.

**Relationship Search search types:** only `near` and `without` exist. Do not reference "same chapter" or "appears before" — they would require a new code path.

**Semantic search granularity:** only `sentence` and `paragraph`. There is no `chapter` granularity — don't offer one in UI without extending `semantic.rs`.

When filling in any of the above, update THIS section as part of the same commit so the list stays accurate.

## Spec

`spec.md` at the project root is the **original design document** from the project's first week. It describes the MVP scope, post-MVP roadmap, entity relationship search, analytics, and IDE-inspired features as originally envisioned. **It runs ahead of the code.** Treat it as aspirational — the product shape the author is building toward — and treat the "Work-in-progress" section above as the ground truth for what actually exists today. When the two disagree, the code wins, CLAUDE.md is updated to match, and spec.md is left alone as the vision document.
