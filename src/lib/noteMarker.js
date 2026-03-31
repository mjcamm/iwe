import { Node, mergeAttributes } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const commentHoverKey = new PluginKey('commentHover');

/**
 * TipTap node extension for inline note markers.
 * Creates a small draggable icon in the document flow.
 * Attributes:
 *   - commentId: DB row ID
 *   - highlightLen: chars after marker to highlight on hover (0 = pinned, no highlight)
 */
export const NoteMarker = Node.create({
  name: 'noteMarker',
  group: 'inline',
  inline: true,
  atom: true,
  draggable: true,

  addAttributes() {
    return {
      commentId: { default: null },
      highlightLen: { default: 0 },
    };
  },

  parseHTML() {
    return [{ tag: 'note-marker' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['note-marker', mergeAttributes(HTMLAttributes)];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement('span');
      dom.className = 'note-marker-icon';
      dom.contentEditable = 'false';
      dom.draggable = true;
      dom.dataset.commentId = node.attrs.commentId;
      dom.title = 'Click to view note';

      // Icon
      const icon = document.createElement('i');
      icon.className = 'bi bi-chat-left-text-fill';
      dom.appendChild(icon);

      // Hover: highlight the text range after this marker
      dom.addEventListener('mouseenter', () => {
        const pos = getPos();
        if (pos == null) return;
        const len = node.attrs.highlightLen;
        if (len > 0) {
          const tr = editor.state.tr;
          tr.setMeta(commentHoverKey, { pos: pos + 1, len });
          editor.view.dispatch(tr);
        }
      });

      dom.addEventListener('mouseleave', () => {
        const tr = editor.state.tr;
        tr.setMeta(commentHoverKey, { pos: null, len: 0 });
        editor.view.dispatch(tr);
      });

      // Click: open note in panel
      dom.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        const event = new CustomEvent('note-marker-click', {
          detail: { commentId: node.attrs.commentId },
          bubbles: true,
        });
        dom.dispatchEvent(event);
      });

      return {
        dom,
        update(updatedNode) {
          if (updatedNode.type.name !== 'noteMarker') return false;
          dom.dataset.commentId = updatedNode.attrs.commentId;
          return true;
        },
      };
    };
  },

  addProseMirrorPlugins() {
    return [
      // Plugin to render hover highlight decorations
      new Plugin({
        key: commentHoverKey,
        state: {
          init() {
            return { pos: null, len: 0 };
          },
          apply(tr, value) {
            const meta = tr.getMeta(commentHoverKey);
            if (meta !== undefined) return meta;
            // If doc changed, clear hover
            if (tr.docChanged) return { pos: null, len: 0 };
            return value;
          },
        },
        props: {
          decorations(state) {
            const { pos, len } = commentHoverKey.getState(state);
            if (pos == null || len <= 0) return DecorationSet.empty;
            const from = pos;
            const to = Math.min(pos + len, state.doc.content.size);
            if (from >= to) return DecorationSet.empty;
            return DecorationSet.create(state.doc, [
              Decoration.inline(from, to, {
                class: 'comment-hover-highlight',
              }),
            ]);
          },
        },
      }),

      // Plugin to clear highlightLen when a noteMarker is dragged/dropped
      new Plugin({
        appendTransaction(transactions, oldState, newState) {
          // Check if any noteMarker was moved (deleted + re-inserted via drag)
          let needsClear = false;
          for (const tr of transactions) {
            if (tr.getMeta('uiEvent') === 'drop') {
              needsClear = true;
              break;
            }
          }
          if (!needsClear) return null;

          // Find all noteMarker nodes and clear their highlightLen
          const tr = newState.tr;
          let changed = false;
          newState.doc.descendants((node, pos) => {
            if (node.type.name === 'noteMarker' && node.attrs.highlightLen > 0) {
              tr.setNodeMarkup(pos, undefined, { ...node.attrs, highlightLen: 0 });
              changed = true;
            }
          });
          return changed ? tr : null;
        },
      }),
    ];
  },
});
