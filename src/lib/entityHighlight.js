import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const entityHighlightKey = new PluginKey('entityHighlight');
export const spellCheckKey = new PluginKey('spellCheck');
export const debugDecoKey = new PluginKey('debugDeco');

/**
 * Create a plugin for debug decorations (dialogue highlighting etc.)
 * Ranges are applied via applyDebugDecorations().
 */
export function createDebugPlugin() {
  return new Plugin({
    key: debugDecoKey,
    state: {
      init() { return DecorationSet.empty; },
      apply(tr, old) {
        const meta = tr.getMeta(debugDecoKey);
        if (meta) return meta;
        return old.map(tr.mapping, tr.doc);
      },
    },
    props: {
      decorations(state) { return this.getState(state); },
    },
  });
}

/**
 * Apply debug decorations to the editor.
 * @param {Editor} editor - TipTap editor instance
 * @param {Array<{from: number, to: number, class?: string}>} ranges - PM position ranges to highlight
 */
export function applyDebugDecorations(editor, ranges) {
  if (!editor || !editor.view) return;
  const doc = editor.state.doc;
  const decos = ranges.map(r =>
    Decoration.inline(r.from, r.to, { class: r.class || 'debug-highlight' })
  );
  const set = DecorationSet.create(doc, decos);
  editor.view.dispatch(editor.state.tr.setMeta(debugDecoKey, set));
}

/**
 * Walk the ProseMirror doc and build plain text + a position map
 * from char index to PM doc position.
 */
export function buildTextMap(doc) {
  let text = '';
  const posMap = [];

  doc.descendants((node, pos) => {
    if (node.isText) {
      for (let i = 0; i < node.text.length; i++) {
        posMap.push(pos + i);
        text += node.text[i];
      }
    }
  });

  return { text, posMap };
}

/**
 * Push decorations into the editor via a transaction.
 */
export function applyDecorations(editor, matches, posMap, viewedEntityIds) {
  if (!editor || !editor.view) return;

  const doc = editor.state.doc;
  const hasViewed = viewedEntityIds.size > 0;
  const decorations = [];

  for (const match of matches) {
    if (match.start >= posMap.length) continue;

    const from = posMap[match.start];
    const toIdx = match.end - 1;
    const to = toIdx < posMap.length ? posMap[toIdx] : null;

    if (from == null || to == null || from < 0 || to < 0) continue;
    const toPos = to + 1;
    if (from >= toPos) continue;

    const isViewed = viewedEntityIds.has(match.entity_id);
    const opacity = !hasViewed ? 0.15 : (isViewed ? 0.35 : 0.06);
    const hexOpacity = Math.round(opacity * 255).toString(16).padStart(2, '0');

    decorations.push(
      Decoration.inline(from, toPos, {
        class: `entity-mark entity-type-${match.entity_type}`,
        style: `background-color: ${match.color}${hexOpacity}; border-bottom: 2px solid ${match.color}${hasViewed && !isViewed ? '40' : ''}; cursor: pointer;${isViewed ? ' border-radius: 2px;' : ''}`,
      }, {
        entityId: match.entity_id,
        entityName: match.entity_name,
      })
    );
  }

  const tr = editor.state.tr;
  tr.setMeta(entityHighlightKey, {
    decorations: DecorationSet.create(doc, decorations),
  });
  editor.view.dispatch(tr);
}

/**
 * Apply spell-error decorations to misspelled words.
 * `misspelled` is an array of { word, start, end } where start/end are char indices.
 * `posMap` is the same posMap from buildTextMap.
 * `entityRanges` is a Set of char indices covered by entity decorations (to skip).
 */
export function applySpellDecorations(editor, misspelled, posMap, entityRanges) {
  if (!editor || !editor.view) return;

  const doc = editor.state.doc;
  const decorations = [];

  for (const item of misspelled) {
    if (item.start >= posMap.length) continue;

    // Skip words that overlap entity decorations
    let overlapsEntity = false;
    for (let i = item.start; i < item.end && i < posMap.length; i++) {
      if (entityRanges.has(i)) {
        overlapsEntity = true;
        break;
      }
    }
    if (overlapsEntity) continue;

    const from = posMap[item.start];
    const toIdx = item.end - 1;
    const to = toIdx < posMap.length ? posMap[toIdx] : null;

    if (from == null || to == null || from < 0 || to < 0) continue;
    const toPos = to + 1;
    if (from >= toPos) continue;

    decorations.push(
      Decoration.inline(from, toPos, {
        class: 'spell-error',
        nodeName: 'span',
      }, {
        spellError: true,
        word: item.word,
      })
    );
  }

  const tr = editor.state.tr;
  tr.setMeta(spellCheckKey, {
    decorations: DecorationSet.create(doc, decorations),
  });
  editor.view.dispatch(tr);
}

/**
 * Create the ProseMirror plugin that holds and renders decorations.
 */
export function createHighlightPlugin(onEntityClick) {
  return new Plugin({
    key: entityHighlightKey,

    state: {
      init() {
        return DecorationSet.empty;
      },
      apply(tr, decorationSet) {
        const meta = tr.getMeta(entityHighlightKey);
        if (meta?.decorations !== undefined) {
          return meta.decorations;
        }
        return decorationSet.map(tr.mapping, tr.doc);
      },
    },

    props: {
      decorations(state) {
        return this.getState(state);
      },

      handleDOMEvents: {
        mousedown(view, event) {
          if (!onEntityClick) return false;

          const coords = { left: event.clientX, top: event.clientY };
          const pos = view.posAtCoords(coords);
          if (!pos) return false;

          const decos = entityHighlightKey.getState(view.state);
          if (!decos) return false;

          const found = decos.find(pos.pos, pos.pos);
          if (found.length > 0 && found[0].spec.entityId) {
            const isCtrl = event.ctrlKey || event.metaKey;
            const entityId = found[0].spec.entityId;
            const entityName = found[0].spec.entityName;

            setTimeout(() => {
              onEntityClick(entityId, entityName, isCtrl);
            }, 0);

            if (isCtrl) {
              event.preventDefault();
              return true;
            }
          }
          return false;
        },
      },
    },
  });
}

/**
 * Create the spell-check decoration plugin.
 */
export function createSpellCheckPlugin() {
  return new Plugin({
    key: spellCheckKey,

    state: {
      init() {
        return DecorationSet.empty;
      },
      apply(tr, decorationSet) {
        const meta = tr.getMeta(spellCheckKey);
        if (meta?.decorations !== undefined) {
          return meta.decorations;
        }
        // On doc change, clear spell decorations immediately.
        // The Editor component handles fade-out via a CSS class before this fires,
        // and fade-in via CSS animation when new decorations arrive.
        if (tr.docChanged) {
          return DecorationSet.empty;
        }
        return decorationSet;
      },
    },

    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  });
}
