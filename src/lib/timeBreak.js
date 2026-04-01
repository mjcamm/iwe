import { Node } from '@tiptap/core';

export const TimeBreak = Node.create({
  name: 'timeBreak',
  group: 'block',
  atom: true,
  draggable: true,
  selectable: true,

  addAttributes() {
    return {
      label: { default: '' },
    };
  },

  parseHTML() {
    return [{
      tag: 'div[data-time-break]',
      getAttrs: dom => ({
        label: dom.getAttribute('data-label') || '',
      }),
    }];
  },

  renderHTML({ node }) {
    return ['div', {
      'data-time-break': '',
      'data-label': node.attrs.label,
      class: 'time-break',
    }, node.attrs.label || 'Time break'];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement('div');
      dom.className = 'time-break';
      dom.setAttribute('contenteditable', 'false');
      dom.setAttribute('data-time-break', '');

      const lineBefore = document.createElement('div');
      lineBefore.className = 'time-break-line';

      const labelEl = document.createElement('span');
      labelEl.className = 'time-break-label';
      labelEl.textContent = node.attrs.label || 'Time break';
      labelEl.setAttribute('contenteditable', 'true');
      labelEl.setAttribute('spellcheck', 'false');

      labelEl.addEventListener('blur', () => {
        const pos = getPos();
        if (pos == null) return;
        const newLabel = labelEl.textContent.trim() || 'Time break';
        if (newLabel !== node.attrs.label) {
          editor.view.dispatch(
            editor.state.tr.setNodeMarkup(pos, undefined, {
              ...node.attrs,
              label: newLabel,
            })
          );
        }
      });

      labelEl.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          labelEl.blur();
        }
      });

      const lineAfter = document.createElement('div');
      lineAfter.className = 'time-break-line';

      dom.appendChild(lineBefore);
      dom.appendChild(labelEl);
      dom.appendChild(lineAfter);

      return {
        dom,
        update(updatedNode) {
          if (updatedNode.type.name !== 'timeBreak') return false;
          labelEl.textContent = updatedNode.attrs.label || 'Time break';
          return true;
        },
        stopEvent(event) {
          // Allow editing the label text
          return event.target === labelEl || labelEl.contains(event.target);
        },
        ignoreMutation(mutation) {
          // Ignore mutations inside the label (user editing)
          return labelEl.contains(mutation.target);
        },
      };
    };
  },

  addCommands() {
    return {
      insertTimeBreak: (attrs = {}) => ({ chain }) => {
        return chain().insertContent({
          type: 'timeBreak',
          attrs: { label: attrs.label || '' },
        }).run();
      },
    };
  },

  addKeyboardShortcuts() {
    return {
      // Delete the time break when backspace is pressed right after it
      'Backspace': ({ editor }) => {
        const { $anchor } = editor.state.selection;
        const before = $anchor.nodeBefore;
        if (before?.type.name === 'timeBreak') {
          const pos = $anchor.pos - before.nodeSize;
          editor.view.dispatch(
            editor.state.tr.delete(pos, $anchor.pos)
          );
          return true;
        }
        return false;
      },
    };
  },
});
