# IWE — Integrated Writing Environment

## The Pitch

An IDE for novelists. The same "click a symbol, see its definition, find all references" experience that programmers take for granted — applied to prose. Characters, places, and things are the symbols. The manuscript is the codebase.

No AI. No generation. No opinions about your writing. Just a text editor that understands the structure of your story and helps you keep track of it.

---

## Stack

- **Frontend:** Svelte 5 (runes)
- **Backend:** Tauri (Rust)
- **Database:** SQLite — one `.iwe` file per project
- **Editor:** TipTap or ProseMirror (rich text with custom decorations)

---

## Core Concepts

### The Manuscript
The novel is stored as **chapters**, each containing raw text as a blob. Chapters are ordered and form the full manuscript. The writer works in one chapter at a time in the editor, with tabs for multiple open chapters.

### Entities
An entity is anything the writer wants to track: a **character**, a **place**, or a **thing**. Each entity has:
- A name (primary)
- Aliases (nicknames, titles, alternate spellings — "Jim", "James", "Mr. Carter", "the detective")
- A type (character / place / thing — extensible later)
- A free-form notes/description field
- Custom key-value fields (eye colour, age, population, whatever the writer needs)

### Negative Entities (Ignored Words)
Words the writer has explicitly dismissed as not being entities. These are suppressed from future auto-detection suggestions. Built up naturally as the writer triages suggestions — every "no" becomes a negative entity so the system never asks again.

Examples of common ignored words: "Monday", "English", "God", "I", month names, languages, nationalities.

### Mentions
The system continuously scans the active chapter text for known entity names and aliases. Each match is a **mention** — a link between an entity and a position in the manuscript. Mentions power everything: the sidebar counts, the reference lists, the highlights in the editor, and the relationship search.

### The Live Link
This is the thing nobody else has built. The editor and the entity database are **aware of each other in real time**:
- Entity names are highlighted in the editor text as you type (like syntax highlighting)
- The sidebar updates mention counts live
- Click a highlighted name in the text → opens the entity card
- Click an entity in the sidebar → shows all mentions with surrounding context

---

## Database Schema (SQLite)

```sql
-- The project itself
CREATE TABLE project (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Chapters in order
CREATE TABLE chapters (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Characters, places, things
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL CHECK(entity_type IN ('character', 'place', 'thing')),
    description TEXT DEFAULT '',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Alternate names for entities ("Jim" → James, "the pub" → The Red Lion)
CREATE TABLE aliases (
    id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    alias TEXT NOT NULL
);

-- Custom fields per entity (key-value, flexible)
CREATE TABLE entity_fields (
    id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    field_name TEXT NOT NULL,
    field_value TEXT DEFAULT ''
);

-- Indexed mentions (rebuilt on scan)
CREATE TABLE mentions (
    id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    chapter_id INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,        -- character offset in chapter content
    context TEXT NOT NULL,             -- surrounding ~150 chars for preview
    matched_text TEXT NOT NULL         -- the actual text that matched (name or alias)
);

-- Words explicitly dismissed as not-entities (negative entities)
CREATE TABLE ignored_words (
    id INTEGER PRIMARY KEY,
    word TEXT NOT NULL UNIQUE
);

-- Bookmarks (writer-placed markers in the manuscript)
CREATE TABLE bookmarks (
    id INTEGER PRIMARY KEY,
    chapter_id INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    label TEXT DEFAULT '',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_mentions_entity ON mentions(entity_id);
CREATE INDEX idx_mentions_chapter ON mentions(chapter_id);
CREATE INDEX idx_aliases_entity ON aliases(entity_id);
```

---

## UI Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  Toolbar: [Project Title]    [Quick Open: Ctrl+P]  [Stats] [⚙]     │
│  Breadcrumb: Part 2 > Chapter 7 > Scene 3                          │
├────────────┬─────────────────────────────────────┬───────────────────┤
│            │ [Ch 4] [Ch 12] [Ch 7 ●]             │                   │
│  Chapter   │                                     │  Entity Panel     │
│  Navigator │         Editor                      │                   │
│            │                                     │  - Card view      │
│  Ch 1      │  She walked into the Red Lion       │    (when one      │
│  Ch 2      │  and found James sitting alone      │     selected)     │
│  Ch 3      │  at the bar. He looked tired,       │                   │
│  Ch 4      │  older than she remembered.         │  - List view      │
│  Ch 5      │                                     │    (default)      │
│  Ch 6      │  "You look terrible," Sarah         │                   │
│  Ch 7  ●   │  said.                              │  - Search view    │
│            │                                     │    (relationship  │
│            │                   ░░▓░░░▓▓░░░░░▓░   │     queries)      │
│            │                   Minimap            │                   │
├────────────┴─────────────────────────────────────┴───────────────────┤
│  Status: Chapter 7 · 3,847 words · 4 characters in scene · [◀ ▶]   │
└──────────────────────────────────────────────────────────────────────┘
```

### Left Panel — Chapter Navigator
- List of chapters, drag to reorder
- Click to switch the editor to that chapter (opens as a tab)
- Shows word count per chapter
- Visual indicator for active chapter
- Right-click: add, rename, delete chapter

### Centre — Editor
- Clean rich text editor (TipTap/ProseMirror)
- **Tabbed chapters** — multiple chapters open at once, click tabs to switch, preserves scroll position
- Entity names highlighted with subtle underlines or background colours
    - Characters in one colour, places in another, things in a third
- Click a highlighted entity name → opens its card in the right panel
- Right-click a highlighted entity name → context menu: "Go to definition", "Find all references", "Ignore this word"
- Right-click an unrecognised capitalised word → "Create entity", "Ignore this word"
- Standard writing features: bold, italic, headings, scene breaks
- Distraction-free mode: hides both sidebars
- **Minimap** — thin scrollbar preview showing entity highlights as coloured dots, visual overview of where characters cluster in a chapter
- **Breadcrumbs** — shows current location in manuscript structure (Part > Chapter > Scene)
- **Navigation history** — back/forward buttons (like alt-left/alt-right in an IDE) to jump back after following a mention link
- **Peek** — alt-click or hover an entity name to see a small inline popup with the card summary without opening the full panel
- **Autocomplete** — typing "Sar" suggests "Sarah" from the entity list, keeps spelling consistent

### Right Panel — Entity Panel

**List View (default):**
- All entities grouped by type (Characters / Places / Things)
- Each shows name + total mention count across manuscript
- Search/filter bar at top
- Click an entity → switches to card view
- "+" button to create new entity

**Card View (when entity selected):**
- Entity name (editable)
- Type badge
- Aliases list (editable, add/remove)
- Description field (free text, markdown)
- Custom fields (add your own key-value pairs)
- **Mentions tab:** Every occurrence in the manuscript shown as a **text snippet** with ~1 line of surrounding context, grouped by chapter. Click any snippet → editor opens that chapter tab and **jumps to that exact position**.
- **Stats tab:** First appearance, last appearance, mention count per chapter, density heatmap

**Search View (relationship queries):**
- See "Entity Relationship Search" section below

---

## Key Behaviours

### Live Scanning
- On every pause in typing (debounce ~500ms), scan the current chapter text for all known entity names and aliases
- Use case-insensitive matching but respect word boundaries (don't match "the" inside "other")
- Update the editor decorations (highlights) and the sidebar mention counts
- Store results in the mentions table for cross-chapter queries

### Full Rescan
- When entities or aliases are added/renamed/deleted, rescan all chapters
- When opening a project, do a full scan on load
- This should be fast — it's just string matching across text, even 100k words scans in milliseconds

### Entity Auto-Detection
- "Detect entities" button that scans the entire manuscript for capitalised words appearing 3+ times that are not at sentence-starting positions (after full stops, question marks, exclamation marks, opening quote marks, em dashes, colons)
- Handles multi-word sequences of capitalised words as potential single entities ("The Red Lion", "Mount Doom", "New York")
- Filters out words already in the entities table and the ignored_words table
- Presents results as a triage list: the writer confirms each suggestion as an entity (pick a type) or dismisses it (added to ignored_words)
- Should also be available as a right-click option on any word in the editor: "Create entity" or "Ignore this word"

### Entity Creation Flow
- Writer can create entities manually from the sidebar (+ button)
- Writer can right-click any word in the editor and create an entity from it
- Auto-detection suggests entities from manuscript scanning
- Dismissed suggestions are stored as negative entities and never suggested again
- Ignored words list is viewable and editable in settings

### Snippet & Jump Navigation
- Every mention in an entity card shows a **snippet**: the matched text with surrounding context (~1 line either side)
- Snippets are for **quick scanning** — skim through 20 mentions without leaving the card
- Each snippet is **clickable** — click it and the editor opens the relevant chapter (as a new tab if not already open) and scrolls to the exact position
- **Navigation history** tracks every jump so the writer can hit back to return to where they were writing
- Flow: writing in Ch 12 → click Sarah in sidebar → skim snippets → click one from Ch 4 → Ch 4 opens at that line → read/edit → hit back → back in Ch 12 exactly where she left off

### Find and Replace (Entity-Aware)
- Standard find and replace across the manuscript
- When renaming text that matches an entity name, offers to update the entity card and all aliases too
- "Rename symbol" — rename an entity from its card and update every occurrence in the manuscript

---

## Entity Relationship Search

A structured search tool for finding narrative connections between entities. No query language — just a simple UI with three elements:

```
[Entity A  ▼]  [relationship  ▼]  [Entity B  ▼]
                                   [distance slider ──●──── ]
```

### Relationship Types (middle dropdown)
- **appears before** — Entity A is mentioned, then Entity B appears later in the same chapter. Returns the text slab between the two mentions.
- **appears near** — both entities mentioned within the same paragraph or within a configurable distance.
- **appears without** — Entity A is present in a chapter/scene but Entity B is absent.
- **appears in same chapter as** — both entities have mentions in the same chapter.

### Distance Slider
- Adjusts how close the two mentions need to be
- "Nearby" = same paragraph, "Far" = anywhere in the same chapter
- Filters results by character count between the two mention positions

### Results
- Displayed as text slabs with both entity names highlighted
- Click any result to jump to that position in the manuscript
- Sorted by distance (tightest connections first by default)

### Implementation
All queries run against the mentions table. "Appears before" is: find mention of entity A, then the next mention of entity B where `B.position > A.position` in the same chapter. Return the text between those positions. Simple SQL joins.

---

## IDE-Inspired Features

Features borrowed directly from programmer IDE patterns:

| IDE Feature | IWE Equivalent |
|---|---|
| Go to definition | Click entity name → opens entity card |
| Find all references | Entity card shows all mentions with context |
| Rename symbol | Rename entity → updates all occurrences in manuscript |
| Quick open (Ctrl+P) | Fuzzy search across chapter titles, entity names, and text content |
| Breadcrumbs | Shows current position: Part > Chapter > Scene |
| Back/Forward navigation | Jump history after following mention links |
| Minimap | Thin scrollbar preview with entity highlights as coloured dots |
| Split editor | Two chapters side by side, or chapter + entity card |
| Bookmarks | Writer-placed markers with labels, listed in a panel, click to jump |
| Peek | Hover/alt-click entity name for inline popup card summary |
| Autocomplete | Suggests entity names as you type for consistent spelling |
| Problems panel | Orphan entities, missing descriptions, long chapters, entities with no mentions |
| Outline view | Scene breaks and entity presence within a chapter |
| Diff view | Compare versions/drafts, additions in green, deletions in red |
| Unused symbol warning | Entity exists in database but has zero mentions in manuscript |

---

## Analytics & Insights

Data derived purely from the mentions table — counting, grouping, displaying:

- **First/last appearance** — which chapter an entity first shows up and where they were last seen
- **Density heatmap** — visual strip along the chapter list showing where each entity is concentrated
- **Chapter cast list** — for each chapter, auto-generated list of every entity present
- **Frequency timeline** — line chart per entity showing mentions per chapter across the manuscript
- **Orphan detection** — entities with zero mentions (created but never written into the story)
- **Compare entities** — select two entities and see side-by-side where they overlap, mention count comparison
- **Entity groups** — custom tags ("the family", "villains", "London locations") with group-level stats
- **Proximity mapping** — which entities frequently appear near each other, surfacing implicit relationships
- **Voice fingerprinting** (future) — per-character dialogue stats: avg sentence length, vocabulary, contractions, question frequency

---

## MVP Scope (v0.1)

Build just enough to be usable:

1. **Chapter navigation** — create, rename, reorder, delete chapters
2. **Editor** — TipTap-based rich text editor with chapter tabs, saves to SQLite
3. **Entity CRUD** — create characters/places/things with names, aliases, description
4. **Negative entities** — ignored words list, dismiss from suggestions or right-click menu
5. **Live name highlighting** — entity names highlighted in editor text
6. **Click-to-card** — click highlighted name → opens entity card in sidebar
7. **Find all references** — entity card shows all mentions as snippets with context, click to jump to location
8. **Navigation history** — back/forward after jumping to mentions
9. **Basic stats** — word count per chapter and total

That's a usable writing tool. Everything else is iteration.

---

## Post-MVP Roadmap

### v0.2 — Detection & Search
- Entity auto-detection (capitalised word scanning with triage UI)
- Quick open (Ctrl+P fuzzy search across everything)
- Find and replace with entity awareness

### v0.3 — Relationship Search
- Entity relationship search UI (appears before/near/without/same chapter)
- Distance slider
- Chapter cast lists

### v0.4 — Analytics
- First/last appearance tracking
- Density heatmaps
- Frequency timelines
- Orphan detection
- Problems panel

### v0.5 — Polish
- Minimap with entity highlights
- Breadcrumbs
- Peek popup on entity hover
- Autocomplete entity names
- Split editor
- Bookmarks
- Distraction-free mode
- Dark mode

---

## File Format

Each project is a single `.iwe` file which is just a SQLite database. This means:
- Fully portable — copy the file, you've copied the project
- No cloud dependency
- Easy backups
- Could be opened by any SQLite browser for data recovery
- Tauri's file dialog handles open/save/save-as naturally

---


## Getting Started

With an empty Tauri + Svelte 5 app already set up:

1. **Add SQLite support** — use `tauri-plugin-sql` or invoke Rust-side SQLite via commands
2. **Set up the schema** — run the CREATE TABLE statements on first project creation
3. **Build the three-panel layout** — Svelte components for navigator, editor, entity panel
4. **Integrate TipTap** — `@tiptap/core` with Svelte adapter, get basic editing working
5. **Entity CRUD** — forms in the right panel, stored in SQLite
6. **The scanner** — a function that takes chapter text + entity list, returns matches with positions
7. **Wire it together** — scanner runs on editor changes, results drive both TipTap decorations and sidebar state
8. **Add chapter tabs** — tab bar above the editor, track scroll position per tab
9. **Add navigation history** — push to a stack on every jump, back/forward buttons pop the stack

The scanner is the heart of the app. Everything else is UI around it.