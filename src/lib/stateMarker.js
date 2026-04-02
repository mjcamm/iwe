import { Node } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const stateMarkerDecoKey = new PluginKey('stateMarkerDecorations');

export const StateMarker = Node.create({
  name: 'stateMarker',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  draggable: true,

  addAttributes() {
    return {
      stateId: { default: null },
      stateType: { default: 'fact' }, // 'fact' or 'relationship'
    };
  },

  parseHTML() {
    return [{
      tag: 'span[data-state-marker]',
      getAttrs: dom => ({
        stateId: parseInt(dom.getAttribute('data-state-id')) || null,
        stateType: dom.getAttribute('data-state-type') || 'fact',
      }),
    }];
  },

  renderHTML({ node }) {
    return ['span', {
      'data-state-marker': '',
      'data-state-id': node.attrs.stateId,
      'data-state-type': node.attrs.stateType,
      class: 'state-marker-anchor',
    }];
  },

  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement('span');
      dom.className = 'state-marker-anchor';
      dom.setAttribute('contenteditable', 'false');
      dom.setAttribute('data-state-marker', '');
      dom.setAttribute('data-state-id', node.attrs.stateId);
      dom.setAttribute('data-state-type', node.attrs.stateType);

      const icon = document.createElement('span');
      icon.className = 'state-marker-icon';
      icon.textContent = '\u25C6';
      icon.title = 'State Marker';
      dom.appendChild(icon);

      return {
        dom,
        update(updatedNode) {
          if (updatedNode.type.name !== 'stateMarker') return false;
          dom.setAttribute('data-state-id', updatedNode.attrs.stateId);
          dom.setAttribute('data-state-type', updatedNode.attrs.stateType);
          icon.title = 'State Marker';
          return true;
        },
        stopEvent() { return false; },
      };
    };
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: stateMarkerDecoKey,
        state: {
          init() {
            return { activeId: null };
          },
          apply(tr, value) {
            const meta = tr.getMeta(stateMarkerDecoKey);
            if (meta !== undefined) return meta;
            return value;
          },
        },
        props: {
          handleClick(view, pos) {
            const $pos = view.state.doc.resolve(pos);
            // Check if we clicked on or right next to a stateMarker node
            const nodeBefore = $pos.nodeBefore;
            const nodeAfter = $pos.nodeAfter;
            const marker = (nodeBefore?.type.name === 'stateMarker' && nodeBefore) ||
                           (nodeAfter?.type.name === 'stateMarker' && nodeAfter);
            if (marker && marker.attrs.stateId != null) {
              view.dom.dispatchEvent(new CustomEvent('state-marker-click', {
                detail: { stateId: marker.attrs.stateId, stateType: marker.attrs.stateType },
                bubbles: true,
              }));
              return true;
            }
            return false;
          },
        },
      }),
    ];
  },
});

/**
 * Set the active state marker (for visual highlighting).
 */
export function setActiveStateMarker(editor, stateId) {
  if (!editor || !editor.view) return;
  const tr = editor.state.tr;
  tr.setMeta(stateMarkerDecoKey, { activeId: stateId });
  editor.view.dispatch(tr);
}
