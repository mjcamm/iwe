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
- **Backend:** Tauri v2 (Rust), rusqlite with bundled SQLite
- **Scanner:** Aho-Corasick (Rust) for blazing-fast multi-pattern entity matching
- **PDF Export:** genpdf with embedded Liberation Serif fonts
- **DOCX Export:** docx npm package
- **Styling:** Bootstrap 5 + custom CSS theme (`src/lib/theme.css`), Bootstrap Icons
- **Drag & Drop:** svelte-dnd-action

## Architecture

### All database access goes through Rust

There is NO `tauri-plugin-sql`. All SQLite operations are Rust commands invoked via `@tauri-apps/api/core`. The frontend JS never touches SQL directly.

**Data flow:** Svelte component → `src/lib/db.js` (thin `invoke()` wrappers) → Tauri command in `src-tauri/src/lib.rs` → database function in `src-tauri/src/db.rs` → rusqlite

### Entity scanning is Rust-side

The Aho-Corasick scanner lives in `src-tauri/src/scanner.rs`. It reads entities from the DB, builds search patterns, and scans text in a single pass. The JS side sends text and receives match results. Never implement scanning in JS.

### Editor reactivity pattern (critical)

TipTap lives outside Svelte's reactivity. The official Svelte 5 pattern is used:

```js
let editorState = $state({ editor: null });
// In onTransaction:
editorState = { editor }; // reassign the OBJECT to trigger reactivity
```

Entity highlight decorations are managed via a ProseMirror plugin (`entityHighlight.js`). Scan results and viewed entity IDs live in Svelte `$state`. A `$effect` watches them and calls `applyDecorations()` which dispatches a PM transaction. An `applyingDecorations` flag prevents infinite loops (decoration transaction → onTransaction → editorState reassign → $effect).

### Navigation / Jump-to-Position (CRITICAL — read this carefully)

`handleGoToChapter(chapterId, searchText, anchorOrPosition)` in the project page is the **ONE central function** for ALL click-to-navigate-and-highlight across the entire app. Every feature that lets the user click a result and jump to a position in the editor MUST use this function. Do not create alternative navigation paths.

**Used by:** find-all-references, text search, dialogue search, relationship search, similar phrasing, word frequency browser, cluster finder, entity detection go-to, pinned excerpt clicks, chapter analysis.

**How it works (two strategies):**

1. **Preferred — `char_position` (number):** When `anchorOrPosition` is a number, it's a char offset from the Rust scanner's plain text. `buildTextMap()` converts it to a ProseMirror doc position via a position map. This is exact and reliable.

2. **Fallback — word-sequence search (string):** When `anchorOrPosition` is a string (anchor text), the function extracts alphanumeric words from it, builds a word list from the PM doc, and finds the matching word sequence. This avoids issues with quotes, dashes, and special characters that differ between Rust's stripped text and PM's DOM representation.

**The text alignment problem:** Rust's `strip_html()` and JS's `buildTextMap()` MUST produce identical text. `buildTextMap()` concatenates text nodes with NO separators between blocks. If you add newlines or spaces between blocks in `buildTextMap()`, char positions from Rust will drift and highlights will land on wrong words. This was a major bug that took multiple iterations to fix.

**When adding new features that navigate:**
- If the Rust result has a `char_position` field, pass it as the third argument (number)
- If you only have text context, pass it as a string — the word-sequence search will handle quotes/punctuation
- ALWAYS call `handleGoToChapter()` — never implement your own text search or position mapping
- The function also handles: opening the chapter tab, scroll-to-center, and the yellow flash highlight box

**Flash highlight:** After jumping, `flashJumpHighlight()` creates overlay `<div>` elements positioned via `Range.getClientRects()` (handles multi-line wrapping). The flash animates for 7 seconds with a blink effect then fades.

**Position mapping detail:** `buildTextMap(doc)` walks the PM doc and builds two things: `text` (plain string) and `posMap` (array where `posMap[charIndex]` = PM doc position). When a `char_position` comes from Rust, `posMap[char_position]` gives the exact PM position for `setTextSelection`.

### Multi-window architecture

Pop-up windows (heatmap, chapter analysis, stats) are separate SvelteKit routes opened via `WebviewWindow` from `@tauri-apps/api/webviewWindow`. They read data directly from the database — no data passing between windows. Each window needs its route in `src/routes/` and its label pattern in `src-tauri/capabilities/default.json`.

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
│   ├── db.js                     # ALL Tauri command wrappers (~47 functions)
│   ├── entityHighlight.js        # ProseMirror decoration plugin
│   ├── export.js                 # DOCX/HTML/TXT/PDF export
│   ├── theme.css                 # Design system CSS variables
│   ├── toast.js                  # Toast notification store
│   └── components/
│       ├── Editor.svelte         # TipTap editor + toolbar + suggestions
│       ├── EntityPanel.svelte    # Entity CRUD + notes + detection
│       ├── ChapterNav.svelte     # Chapter sidebar
│       ├── SearchPanel.svelte    # Text/dialogue/relationship search
│       ├── AnalysisPanel.svelte  # Frequency/clusters/similar/heatmap
│       └── Toasts.svelte         # Toast renderer

src-tauri/
├── src/
│   ├── main.rs                   # Entry point
│   ├── lib.rs                    # ~44 Tauri commands + PDF export
│   ├── db.rs                     # Database schema + all queries
│   ├── scanner.rs                # Aho-Corasick scanner + 13 commands
│   └── wordlists.rs              # Verb/adjective/adverb lists for POS tags
├── fonts/
│   └── LiberationSerif-*.ttf     # Embedded fonts for PDF export
├── capabilities/default.json     # Tauri permissions
└── Cargo.toml
```

## Database Schema (11 tables)

- **chapters** — id, title, content (HTML), sort_order, timestamps
- **entities** — id, name, entity_type (character/place/thing), description, color, visible
- **aliases** — entity_id, alias text
- **entity_fields** — key-value custom fields (schema exists, UI not built)
- **ignored_words** — words excluded from entity detection
- **entity_notes** — pinned text excerpts from chapters, with sort_order
- **entity_free_notes** — free-form note cards per entity, with sort_order
- **writing_activity** — per-save log (timestamp, chapter, word counts, delta)
- **daily_stats** — aggregated daily (words added/deleted/net, active minutes)
- **writing_settings** — daily_goal, session_gap_minutes (singleton)
- **nav_history** — chapter_id, scroll_top, cursor_pos (max 100)

Migrations are handled in `init_schema()` with `ALTER TABLE ... ADD COLUMN` checks for backward compatibility with existing `.iwe` files.

## Key Design Decisions

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

### Writing stats tracking
On every content save (500ms debounce), the project page computes word count delta and calls `logWritingActivity()`. The Rust side updates `daily_stats` atomically. Active time is computed from gaps between consecutive activities (capped at session_gap_minutes).

### Settings storage
App-level settings (projects folder, panel widths) are stored in `settings.json` in Tauri's app data dir via `tauri-plugin-fs`. Project-level settings (writing goals, entity data) are in the `.iwe` SQLite file.

## Common Patterns

### Adding a new Tauri command

1. Add the function to `src-tauri/src/db.rs` (or `scanner.rs`)
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

### Adding a new analysis sub-tab

1. Add state variables in `AnalysisPanel.svelte`
2. Add the sub-tab button in the `analysis-sub-tabs` div
3. Add the `{:else if subTab === 'mytab'}` view
4. If it needs Rust processing, add the command following the pattern above

## Design System

**Colors:** Warm whites (#faf8f5 sidebar, #fffef9 paper), deep literary teal accent (#2d6a5e), warm grays for text hierarchy. Entity type colors: character=teal, place=brown, thing=purple.

**Typography:** Libre Baskerville (serif) for prose/titles, Source Sans 3 (sans) for UI. Loaded via Google Fonts in `theme.css`.

**Entity colors** are user-customizable per entity with a color picker. Defaults by type. Color is used for editor highlights, heatmap cells, entity dots, and search result highlighting.

## User Preferences (from memory)

- **Vanilla JS only** — no TypeScript. All files are `.js`, no `lang="ts"` in Svelte.
- **Do it right** — Rust-side processing preferred over JS workarounds. The full DB migration to rusqlite was done specifically because passing data through JS was "not the right way."
- **Author-focused UX** — features should be framed for writers, not developers. "Scene Heading" not "H3". Word types `{verb}` not regex jargon.

## Spec

The full feature vision is in `spec.md` at the project root. It describes the MVP scope, post-MVP roadmap, entity relationship search, analytics, and IDE-inspired features.
