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

**Two independent PDF paths:**
- `/print/+page.svelte` (popup) is NOT Typst — it converts Y.Doc → ProseMirror JSON → HTML via `yDocToProsemirrorJSON` + `generateHTML`, applies A4 or Book CSS, and triggers `window.print()`. Fast, rough, zero layout control.
- The toolbar `Export → PDF` uses `genpdf` in Rust (`lib.rs:686-823`) with embedded Liberation Serif, A4 or 5×8 book preset. Also independent of Typst.
- The `Formatting` project subroute is the Typst path: `format::compile_preview` compiles a cached `PagedDocument` and exposes pages as SVG via the custom `iwe://localhost/preview/page/{n}.svg` URI scheme (see `lib.rs:1265-1298`). This is the "publication-quality" path. Do not confuse it with the other two.

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
│       ├── PageContentEditor.svelte # Rich HTML editor for format-editor custom pages (with image support)
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
│   ├── format.rs                 # Typst-based Format editor: FormatState, compile_preview, render_page_svg, list_system_fonts
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
- **format_pages** — page_role, title, content (HTML), position ('front'|'back'), sort_order, include_in, vertical_align. Shared across all profiles; the Format editor uses `format_page_exclusions` to hide specific pages per profile.
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
Three sidebar views: **Projects**, **Word Palettes** (renders `WordPalettes.svelte`), **Settings**. First-run flow: pick a projects folder (persisted via `getProjectsDir`/`setProjectsDir` in `db.js`). Projects view lists `.iwe` files from the chosen folder, buttons: New Manuscript, Import…, Refresh, Open Folder (reveals in OS), Change Folder. Delete has a "type `delete` to confirm" modal. A dev-only ⚗ button on each project row launches the `/analyse` popup window (visible only when `import.meta.env.DEV`). Settings view covers: Dictionary language (en_US / en_GB), Typewriter mode, Descriptive Search indexing delay (seconds), Projects folder, Backup interval (minutes), plus license credits for ONNX + all-mpnet-base-v2.

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

### Format & Layout editor — `src/routes/project/[filename]/format/+page.svelte` + `src-tauri/src/format.rs`
The "publication-quality" path. Project subroute. Two-column layout: **Typst-rendered SVG preview (left) + profile sidebar (right, resizable, width persisted to `settings.json`)**.

**How the preview works:**
1. `compile_preview(profile_id)` in `format.rs` loads chapters + format_pages + exclusions, extracts Y.Doc text, builds a Typst markup document via `build_typst_markup`, compiles with the `typst` crate, caches the resulting `PagedDocument` in `FormatState.document`.
2. The frontend receives `page_count` and renders `<img src="http://iwe.localhost/preview/page/{i}.svg?v={generation}">` per page. These URIs go through a custom protocol handler registered in `lib.rs::run` (`register_uri_scheme_protocol("iwe", ...)`) which calls `format::render_page_svg(page_index)`.
3. **Pages lazy-load** via an IntersectionObserver + 150ms scroll-idle commit. Only pages within ±2 of the visible range get their `<img src>` set; the rest show aspect-correct placeholders. Re-observes on page-count or compile-generation change.
4. A **timing bar** at the bottom shows DB load / Y.Doc extract / markup build / Typst compile / total ms (debug aid — keep it).

**Profile management** (`format_profiles` table): dropdown + rename + **Copy settings** (serializes profile minus target/size to `settings.json` clipboard) + **Paste settings** (applies clipboard to active profile via `paste_format_profile_settings`) + **New** (optionally duplicate existing + pick Print/Ebook target) + **Delete** (inline Yes/No, disabled when only one profile remains).

**Sidebar modes:**
- **Pages** — tag bar with 17 `page_role` values (half-title, title, copyright, dedication, epigraph, toc, foreword, preface, prologue, epilogue, afterword, acknowledgments, about-author, also-by, glossary, excerpt, blurbs). Click to arm, then click a page in the list to insert below. Used tags show ×. Three draggable sections: Front Matter, Chapters (locked), Back Matter. Each page has a row of **profile-inclusion pills** that toggle `format_page_exclusions` for per-profile visibility. Full-content editing opens `PageContentEditor.svelte` (rich HTML editor with drag-resizable image support).
- **Target** — preset cards. **Print:** 6×9 Paperback, 5.5×8.5 Paperback, 5×8 Paperback, A5 (5.83×8.27), US Letter. **Ebook:** Kindle (4.5×7.2), EPUB generic (5×7.5), Apple Books (5×7.5).
- **Themes** — placeholder ("Theme presets will appear here"). **Not implemented.**
- **Custom** — dropdown with 8 settings subpanels (each a component in `src/lib/components/format/`): Chapter Headings, Paragraph, Headings, Breaks, Print Layout, Typography, Header/Footer, Trim. Changes flush to `format_profiles.{category}_json` via `update_profile_category`.

**Ebook output** currently shows a placeholder ("Ebook preview coming soon — Ebooks use reflowable HTML — no fixed pages to preview") when a profile's `target_type === 'ebook'`. The compile path returns 0 pages for ebook profiles. **Not implemented yet.**

**Fonts:** `list_system_fonts` uses `typst_kit::fonts::FontSearcher` to discover system fonts, with the four embedded Liberation Serif variants as fallback (`format.rs:15-18`). `FontPicker.svelte` renders each family in its own face for WYSIWYG preview.

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
App-level settings (projects folder, panel widths) are stored in `settings.json` in Tauri's app data dir via `tauri-plugin-fs`. Project-level settings (writing goals, entity data) are in the `.iwe` SQLite file.

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
- **Format editor → Ebook output** — when a profile has `target_type === 'ebook'`, the preview shows "Ebook preview coming soon" and `compile_preview` returns 0 pages. No EPUB generation path.
- **Entity custom fields** — the `entity_fields` table exists in the schema but has zero UI. Don't delete the table; it's reserved.

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
