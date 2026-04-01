import { Node } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const commentDecoKey = new PluginKey('commentDecorations');

export const NoteMarker = Node.create({
  name: 'noteMarker',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: false,

  addAttributes() {
    return {
      commentId: { default: null },
      highlightLen: { default: 0 },
    };
  },

  parseHTML() {
    return [{
      tag: 'span[data-note-marker]',
      getAttrs: dom => ({
        commentId: parseInt(dom.getAttribute('data-comment-id')) || null,
        highlightLen: parseInt(dom.getAttribute('data-highlight-len')) || 0,
      }),
    }];
  },

  renderHTML({ node }) {
    return ['span', {
      'data-note-marker': '',
      'data-comment-id': node.attrs.commentId,
      'data-highlight-len': node.attrs.highlightLen,
      class: 'note-marker-anchor',
    }];
  },

  // Zero-width invisible anchor
  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement('span');
      dom.className = 'note-marker-anchor';
      dom.setAttribute('contenteditable', 'false');
      dom.setAttribute('data-note-marker', '');
      dom.setAttribute('data-comment-id', node.attrs.commentId);
      return {
        dom,
        update(updatedNode) {
          if (updatedNode.type.name !== 'noteMarker') return false;
          dom.setAttribute('data-comment-id', updatedNode.attrs.commentId);
          return true;
        },
        stopEvent() { return false; },
      };
    };
  },

  addProseMirrorPlugins() {
    return [
      // Persistent comment highlight decorations
      new Plugin({
        key: commentDecoKey,
        state: {
          init() {
            return { decorations: DecorationSet.empty, activeId: null };
          },
          apply(tr, value) {
            const meta = tr.getMeta(commentDecoKey);
            if (meta !== undefined) return meta;
            // Map decorations through doc changes
            if (tr.docChanged) {
              return {
                decorations: value.decorations.map(tr.mapping, tr.doc),
                activeId: value.activeId,
              };
            }
            return value;
          },
        },
        props: {
          decorations(state) {
            return commentDecoKey.getState(state)?.decorations || DecorationSet.empty;
          },
          handleClick(view, pos) {
            const decos = commentDecoKey.getState(view.state)?.decorations;
            if (!decos) return false;
            const found = decos.find(pos, pos);
            for (const d of found) {
              if (d.spec?.commentId != null) {
                // Dispatch a custom event for the comment click
                view.dom.dispatchEvent(new CustomEvent('comment-highlight-click', {
                  detail: { commentId: d.spec.commentId },
                  bubbles: true,
                }));
                return false;
              }
            }
            return false;
          },
        },
      }),
    ];
  },
});

/**
 * Build and apply comment highlight decorations.
 * Walks the doc for noteMarker nodes, creates inline decorations for their ranges.
 * `activeCommentId` gets a brighter highlight.
 */
export function applyCommentHighlights(editor, activeCommentId) {
  if (!editor || !editor.view) return;

  const doc = editor.state.doc;
  const decorations = [];

  doc.descendants((node, pos) => {
    if (node.type.name === 'noteMarker' && node.attrs.highlightLen > 0) {
      const from = pos + 1; // after the marker node
      const to = Math.min(from + node.attrs.highlightLen, doc.content.size);
      if (from < to) {
        const isActive = node.attrs.commentId === activeCommentId;
        decorations.push(
          Decoration.inline(from, to, {
            class: isActive ? 'comment-highlight comment-highlight-active' : 'comment-highlight',
          }, {
            commentId: node.attrs.commentId,
          })
        );
      }
    }
  });

  const tr = editor.state.tr;
  tr.setMeta(commentDecoKey, {
    decorations: DecorationSet.create(doc, decorations),
    activeId: activeCommentId,
  });
  editor.view.dispatch(tr);
}
