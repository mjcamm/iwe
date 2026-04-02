import { Node } from '@tiptap/core';

export const TimeBreak = Node.create({
  name: 'timeBreak',
  group: 'block',
  content: 'block+',
  defining: true,

  addAttributes() {
    return {
      label: { default: 'Time jump' },
    };
  },

  parseHTML() {
    return [{
      tag: 'div[data-time-break]',
      getAttrs: dom => ({
        label: dom.getAttribute('data-label') || 'Time jump',
      }),
    }];
  },

  renderHTML({ node, HTMLAttributes }) {
    return ['div', {
      'data-time-break': '',
      'data-label': node.attrs.label,
      class: 'time-break-wrapper',
    }, 0];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      // Outer wrapper
      const dom = document.createElement('div');
      dom.className = 'time-break-wrapper';
      dom.setAttribute('data-time-break', '');

      // === Start divider ===
      const startDiv = document.createElement('div');
      startDiv.className = 'time-break-divider time-break-start';
      startDiv.setAttribute('contenteditable', 'false');

      const startLine1 = document.createElement('div');
      startLine1.className = 'time-break-line';

      const labelEl = document.createElement('span');
      labelEl.className = 'time-break-label';
      labelEl.textContent = node.attrs.label || 'Time jump';
      labelEl.setAttribute('contenteditable', 'true');
      labelEl.setAttribute('spellcheck', 'false');

      labelEl.addEventListener('blur', () => {
        const pos = getPos();
        if (pos == null) return;
        const newLabel = labelEl.textContent.trim() || 'Time jump';
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

      const startLine2 = document.createElement('div');
      startLine2.className = 'time-break-line';

      startDiv.appendChild(startLine1);
      startDiv.appendChild(labelEl);
      startDiv.appendChild(startLine2);

      // === Content hole ===
      const contentDom = document.createElement('div');
      contentDom.className = 'time-break-content';

      // === End divider ===
      const endDiv = document.createElement('div');
      endDiv.className = 'time-break-divider time-break-end';
      endDiv.setAttribute('contenteditable', 'false');

      const endLine1 = document.createElement('div');
      endLine1.className = 'time-break-line';

      const endLabel = document.createElement('span');
      endLabel.className = 'time-break-end-label';
      endLabel.textContent = 'End time jump';

      const endLine2 = document.createElement('div');
      endLine2.className = 'time-break-line';

      endDiv.appendChild(endLine1);
      endDiv.appendChild(endLabel);
      endDiv.appendChild(endLine2);

      dom.appendChild(startDiv);
      dom.appendChild(contentDom);
      dom.appendChild(endDiv);

      return {
        dom,
        contentDOM: contentDom,
        update(updatedNode) {
          if (updatedNode.type.name !== 'timeBreak') return false;
          labelEl.textContent = updatedNode.attrs.label || 'Time jump';
          return true;
        },
        stopEvent(event) {
          return event.target === labelEl || labelEl.contains(event.target);
        },
        ignoreMutation(mutation) {
          return labelEl.contains(mutation.target) || endDiv.contains(mutation.target);
        },
      };
    };
  },

  addCommands() {
    return {
      insertTimeBreak: (attrs = {}) => ({ chain, state }) => {
        // Don't allow nesting — check if cursor is already inside a timeBreak
        const { $from } = state.selection;
        for (let d = $from.depth; d > 0; d--) {
          if ($from.node(d).type.name === 'timeBreak') {
            return false;
          }
        }
        return chain().insertContent({
          type: 'timeBreak',
          attrs: { label: attrs.label || 'Time jump' },
          content: [{ type: 'paragraph' }],
        }).run();
      },
    };
  },

  addKeyboardShortcuts() {
    return {
      // Pressing Enter at the very end of a timeBreak's last block should exit the wrapper
      'Enter': ({ editor }) => {
        const { $head } = editor.state.selection;
        // Check if we're in a timeBreak
        let tbDepth = -1;
        for (let d = $head.depth; d > 0; d--) {
          if ($head.node(d).type.name === 'timeBreak') {
            tbDepth = d;
            break;
          }
        }
        if (tbDepth < 0) return false;

        const tbNode = $head.node(tbDepth);
        const tbPos = $head.before(tbDepth);
        const endOfTb = tbPos + tbNode.nodeSize;

        // Only exit if cursor is at the very end and the last block is empty
        const lastChild = tbNode.lastChild;
        if (lastChild && lastChild.type.name === 'paragraph' && lastChild.content.size === 0) {
          const lastChildPos = tbPos + tbNode.nodeSize - lastChild.nodeSize - 1;
          if ($head.pos >= lastChildPos) {
            // Remove empty paragraph and insert one after the timeBreak
            const tr = editor.state.tr;
            tr.delete(lastChildPos, lastChildPos + lastChild.nodeSize);
            tr.insert(endOfTb - lastChild.nodeSize - 1, editor.state.schema.nodes.paragraph.create());
            editor.view.dispatch(tr);
            // Move cursor to the new paragraph
            editor.commands.focus(endOfTb - lastChild.nodeSize);
            return true;
          }
        }
        return false;
      },
    };
  },
});
