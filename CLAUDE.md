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

**Text extraction:** `ydoc::extract_plain_text()` walks the Y.Doc's `XmlFragment` depth-first, concatenating text from `XmlText` nodes with NO separators between blocks — matching `buildTextMap()` exactly. `extract_text_with_breaks()` inserts `\n\n` between top-level blocks (used for PDF export).

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

### Navigation / Jump-to-Position (CRITICAL — read this carefully)

`handleGoToChapter(chapterId, searchText, anchorOrPosition)` in the project page is the **ONE central function** for ALL click-to-navigate-and-highlight across the entire app. Every feature that lets the user click a result and jump to a position in the editor MUST use this function. Do not create alternative navigation paths.

**Used by:** find-all-references, text search, dialogue search, relationship search, similar phrasing, word frequency browser, cluster finder, entity detection go-to, pinned excerpt clicks, chapter analysis.

**How it works (two strategies):**

1. **Preferred — `char_position` (number):** When `anchorOrPosition` is a number, it's a char offset from the Rust scanner's plain text. `buildTextMap()` converts it to a ProseMirror doc position via a position map. This is exact and reliable — both Rust and JS extract text from the same Y.Doc, so char offsets are guaranteed identical.

2. **Fallback — word-sequence search (string):** When `anchorOrPosition` is a string (anchor text), the function extracts alphanumeric words from it, builds a word list from the PM doc, and finds the matching word sequence. This avoids issues with quotes, dashes, and special characters.

**When adding new features that navigate:**
- If the Rust result has a `char_position` field, pass it as the third argument (number)
- If you only have text context, pass it as a string — the word-sequence search will handle quotes/punctuation
- ALWAYS call `handleGoToChapter()` — never implement your own text search or position mapping
- The function also handles: opening the chapter tab, scroll-to-center, and the yellow flash highlight box

**Position mapping detail:** `buildTextMap(doc)` walks the PM doc and builds two things: `text` (plain string) and `posMap` (array where `posMap[charIndex]` = PM doc position). When a `char_position` comes from Rust, `posMap[char_position]` gives the exact PM position for `setTextSelection`.

### Multi-window architecture

Pop-up windows (heatmap, chapter analysis, stats) are separate SvelteKit routes opened via `WebviewWindow` from `@tauri-apps/api/webviewWindow`. They read data directly from the database — no data passing between windows. Each window needs its route in `src/routes/` and its label pattern in `src-tauri/capabilities/default.json`.

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
│   └── print/                    # Print preview (popup window)
├── lib/
│   ├── db.js                     # ALL Tauri command wrappers
│   ├── entityHighlight.js        # ProseMirror decoration plugin (entities + spellcheck)
│   ├── noteMarker.js             # NoteMarker TipTap node + comment highlight decorations
│   ├── ydoc.js                   # Y.Doc lifecycle (create, encode, destroy)
│   ├── export.js                 # DOCX/HTML/TXT/PDF export
│   ├── theme.css                 # Design system CSS variables
│   ├── toast.js                  # Toast notification store
│   └── components/
│       ├── Editor.svelte         # TipTap editor + toolbar + suggestions + spellcheck + context menu + comments
│       ├── WordModal.svelte      # Combined spelling & synonym modal
│       ├── EntityPanel.svelte    # Entity CRUD + notes + detection
│       ├── NotesPanel.svelte     # Comments/notes list + detail view
│       ├── ChapterNav.svelte     # Chapter sidebar
│       ├── SearchPanel.svelte    # Text/dialogue/relationship search
│       ├── AnalysisPanel.svelte  # Frequency/clusters/similar/heatmap
│       └── Toasts.svelte         # Toast renderer

src-tauri/
├── src/
│   ├── main.rs                   # Entry point
│   ├── lib.rs                    # Tauri commands + PDF export
│   ├── db.rs                     # Database schema + all queries
│   ├── scanner.rs                # Aho-Corasick scanner + commands
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

## Database Schema (13 tables)

- **chapters** — id, title, content (BLOB — Yjs state), sort_order, timestamps
- **entities** — id, name, entity_type (character/place/thing), description, color, visible
- **aliases** — entity_id, alias text
- **entity_fields** — key-value custom fields (schema exists, UI not built)
- **ignored_words** — words excluded from entity detection
- **custom_words** — spell checker custom dictionary (word, source: 'user' or 'entity')
- **entity_notes** — pinned text excerpts from chapters, with sort_order
- **entity_free_notes** — free-form note cards per entity, with sort_order
- **comments** — chapter_id, note_text, timestamps (position lives in the Y.Doc as a noteMarker node)
- **writing_activity** — per-save log (timestamp, chapter, word counts, delta)
- **daily_stats** — aggregated daily (words added/deleted/net, active minutes)
- **writing_settings** — daily_goal, session_gap_minutes (singleton)
- **nav_history** — chapter_id, scroll_top, cursor_pos (max 100)

Migrations are handled in `init_schema()` with `CREATE TABLE IF NOT EXISTS` for idempotent creation and `ALTER TABLE ... ADD COLUMN` checks for backward compatibility with existing `.iwe` files.

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

### Right-click context menu
The editor has a custom context menu (`handleContextMenu` in Editor.svelte) that adapts based on what was right-clicked: entity words get "Go to definition" and "Find references"; misspelled words get "Spelling & Synonyms..." and "Add to dictionary"; all words get "Synonyms..." and entity creation options; "Add a note" is always available (creates a comment highlight on the selection, or a pinned note at cursor if no selection). The WordModal component is a large modal with flowing pill grids for both spelling suggestions and synonyms.

### Decoration coexistence (three independent plugin systems)
Three separate ProseMirror plugins manage decorations independently: `entityHighlightKey` for entity highlights, `spellCheckKey` for red squiggly underlines, and `commentDecoKey` for comment highlights. The `applyingDecorations` flag prevents entity/spell decorations from triggering infinite loops. Each plugin has its own PluginKey and state management.

### Writing stats tracking
On every content save (500ms debounce), the project page computes word count delta and calls `logWritingActivity()`. The Rust side updates `daily_stats` atomically. Active time is computed from gaps between consecutive activities (capped at session_gap_minutes).

### Settings storage
App-level settings (projects folder, panel widths) are stored in `settings.json` in Tauri's app data dir via `tauri-plugin-fs`. Project-level settings (writing goals, entity data) are in the `.iwe` SQLite file.

## Common Patterns

### Adding a new Tauri command

1. Add the function to `src-tauri/src/db.rs` (or `scanner.rs`, `ydoc.rs`)
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

### Adding a new analysis sub-tab

1. Add state variables in `AnalysisPanel.svelte`
2. Add the sub-tab button in the `analysis-sub-tabs` div
3. Add the `{:else if subTab === 'mytab'}` view
4. If it needs Rust processing, add the command following the pattern above

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
