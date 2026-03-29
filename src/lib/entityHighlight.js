import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const entityHighlightKey = new PluginKey('entityHighlight');

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
          console.log('[entity-click] mousedown fired', { ctrlKey: event.ctrlKey, metaKey: event.metaKey, button: event.button });

          if (!onEntityClick) { console.log('[entity-click] no onEntityClick callback'); return false; }

          const coords = { left: event.clientX, top: event.clientY };
          console.log('[entity-click] coords:', coords);

          const pos = view.posAtCoords(coords);
          console.log('[entity-click] posAtCoords result:', pos);
          if (!pos) { console.log('[entity-click] no pos from coords'); return false; }

          const decos = entityHighlightKey.getState(view.state);
          console.log('[entity-click] decorations state:', decos ? `${decos.find(0, view.state.doc.content.size).length} total decos` : 'null');
          if (!decos) { console.log('[entity-click] no decoration state'); return false; }

          const found = decos.find(pos.pos, pos.pos);
          console.log('[entity-click] decos at pos', pos.pos, ':', found.length, 'found');

          if (found.length > 0) {
            console.log('[entity-click] first deco spec:', found[0].spec);
            if (found[0].spec.entityId) {
              const isCtrl = event.ctrlKey || event.metaKey;
              const entityId = found[0].spec.entityId;
              const entityName = found[0].spec.entityName;
              console.log(`[entity-click] MATCH! entity="${entityName}" id=${entityId} isCtrl=${isCtrl}`);

              setTimeout(() => {
                console.log(`[entity-click] firing callback for "${entityName}" isCtrl=${isCtrl}`);
                onEntityClick(entityId, entityName, isCtrl);
              }, 0);

              if (isCtrl) {
                event.preventDefault();
                console.log('[entity-click] prevented default for Ctrl+click');
                return true;
              }
            }
          } else {
            console.log('[entity-click] no entity decoration at click position');
          }
          return false;
        },
      },
    },
  });
}
