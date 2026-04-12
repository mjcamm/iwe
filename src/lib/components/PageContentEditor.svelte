<script>
  import { onMount, onDestroy } from 'svelte';
  import { Editor, Node, mergeAttributes } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import TextAlign from '@tiptap/extension-text-align';
  import { TextStyle, FontSize, FontFamily } from '@tiptap/extension-text-style';
  import Placeholder from '@tiptap/extension-placeholder';
  import FontPicker from '$lib/components/FontPicker.svelte';

  let { page, profile, onsave, oncancel } = $props();

  // Vertical alignment state — sync from page prop on mount, send back on save.
  let verticalAlign = $state(page.vertical_align || 'top');

  // Custom Image node with a drag-resize NodeView. The width attribute is persisted
  // to JSON and flows through to the Typst renderer (which converts px → pt).
  const ImageNode = Node.create({
    name: 'image',
    inline: false,
    group: 'block',
    draggable: true,
    addAttributes() {
      return {
        src: { default: null },
        alt: { default: null },
        width: { default: null }, // string like "300px"
      };
    },
    parseHTML() {
      return [{ tag: 'img[src]' }];
    },
    renderHTML({ HTMLAttributes }) {
      return ['img', mergeAttributes(HTMLAttributes)];
    },
    addNodeView() {
      return ({ node, editor: ed, getPos }) => {
        // Outer = block, centers the inner shrink-to-image element
        const outer = document.createElement('div');
        outer.className = 'image-outer';

        // Inner = inline-block sized to image, anchors the resize handle
        const inner = document.createElement('span');
        inner.className = 'image-frame';

        const img = document.createElement('img');
        img.src = node.attrs.src;
        img.alt = node.attrs.alt || '';
        img.draggable = false;
        if (node.attrs.width) img.style.width = node.attrs.width;

        const handle = document.createElement('span');
        handle.className = 'image-resize-handle';
        handle.contentEditable = 'false';

        handle.addEventListener('mousedown', (e) => {
          e.preventDefault();
          e.stopPropagation();
          const startX = e.clientX;
          const startWidth = img.offsetWidth;

          const onMove = (ev) => {
            const newWidth = Math.max(40, startWidth + (ev.clientX - startX));
            img.style.width = newWidth + 'px';
          };
          const onUp = () => {
            document.removeEventListener('mousemove', onMove);
            document.removeEventListener('mouseup', onUp);
            const finalWidth = img.style.width;
            if (typeof getPos === 'function') {
              const pos = getPos();
              if (pos != null) {
                const tr = ed.state.tr.setNodeMarkup(pos, undefined, {
                  ...node.attrs,
                  width: finalWidth,
                });
                ed.view.dispatch(tr);
              }
            }
          };
          document.addEventListener('mousemove', onMove);
          document.addEventListener('mouseup', onUp);
        });

        inner.appendChild(img);
        inner.appendChild(handle);
        outer.appendChild(inner);

        return {
          dom: outer,
          update: (updatedNode) => {
            if (updatedNode.type !== node.type) return false;
            if (updatedNode.attrs.src !== img.src) {
              img.src = updatedNode.attrs.src;
            }
            if (updatedNode.attrs.width !== img.style.width) {
              img.style.width = updatedNode.attrs.width || '';
            }
            return true;
          },
          selectNode: () => { inner.classList.add('is-selected'); },
          deselectNode: () => { inner.classList.remove('is-selected'); },
        };
      };
    },
  });

  const FONT_SIZES = ['8pt', '9pt', '10pt', '11pt', '12pt', '14pt', '16pt', '18pt', '24pt', '32pt', '48pt'];

  let element;
  let editor = $state(null);
  let editorState = $state({ editor: null });

  // Parse stored content. Empty string → blank doc. JSON → use it. Plain text → wrap in paragraph.
  function parseInitialContent(raw) {
    if (!raw || !raw.trim()) {
      return { type: 'doc', content: [{ type: 'paragraph' }] };
    }
    if (raw.trim().startsWith('{')) {
      try {
        return JSON.parse(raw);
      } catch {}
    }
    // Legacy plain text
    const paragraphs = raw.split('\n\n').map(p => ({
      type: 'paragraph',
      content: p.trim() ? [{ type: 'text', text: p.trim() }] : [],
    }));
    return { type: 'doc', content: paragraphs };
  }

  onMount(() => {
    editor = new Editor({
      element,
      extensions: [
        StarterKit.configure({ heading: false }), // we don't use H1/H2/H3 — font sizes only
        TextStyle,
        FontSize,
        FontFamily,
        ImageNode,
        TextAlign.configure({ types: ['paragraph'] }),
        Placeholder.configure({ placeholder: 'Start writing...' }),
      ],
      content: parseInitialContent(page.content),
      onTransaction: () => {
        editorState = { editor };
      },
      // Strip all formatting on paste — users bring raw text in and style it
      // here using the toolbar. This prevents CSS font-family stacks, inline
      // styles, and other source-app formatting from leaking into the Typst
      // markup builder.
      editorProps: {
        handlePaste(view, event) {
          const plain = event.clipboardData?.getData('text/plain');
          if (plain) {
            editor.commands.insertContent(plain);
            return true;
          }
          return false;
        },
      },
    });
    editorState = { editor };
  });

  // ---- Image upload ----

  let fileInput;

  function triggerImageUpload() {
    fileInput?.click();
  }

  function handleFileSelected(e) {
    const file = e.target.files?.[0];
    if (!file || !editor) return;
    if (!file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result;
      editor.chain().focus().insertContent({
        type: 'image',
        attrs: { src: dataUrl, alt: file.name },
      }).run();
    };
    reader.readAsDataURL(file);
    e.target.value = ''; // reset so the same file can be selected again
  }

  onDestroy(() => {
    if (editor) editor.destroy();
  });

  function handleSave() {
    if (!editor) return;
    const json = editor.getJSON();
    onsave({ content: JSON.stringify(json), verticalAlign });
  }

  function handleCancel() {
    oncancel();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      handleCancel();
    } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
  }

  // Toolbar action helpers (rely on reactive editorState)
  let ed = $derived(editorState.editor);

  // Compute true-to-scale dimensions
  let widthIn = $derived(profile?.trim_width_in ?? 6);
  let heightIn = $derived(profile?.trim_height_in ?? 9);
  let marginTop = $derived(profile?.margin_top_in ?? 0.875);
  let marginBottom = $derived(profile?.margin_bottom_in ?? 0.875);
  let marginOutside = $derived(profile?.margin_outside_in ?? 0.625);
  let marginInside = $derived(profile?.margin_inside_in ?? 0.875);
  let fontSize = $derived(profile?.font_size_pt ?? 11);
  let lineSpacing = $derived(profile?.line_spacing ?? 1.4);
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="page-editor-backdrop" onclick={handleCancel}>
  <div class="page-editor-shell" onclick={(e) => e.stopPropagation()}>
    <div class="editor-header">
      <div class="editor-title">
        <strong>{page.title || 'Untitled page'}</strong>
        <span class="editor-role-badge">{page.page_role}</span>
      </div>
      <div class="editor-header-actions">
        <button class="editor-btn" onclick={handleCancel}>Cancel</button>
        <button class="editor-btn primary" onclick={handleSave}>Save (Ctrl+S)</button>
      </div>
    </div>

    {#if ed}
      <div class="editor-toolbar">
        <button class="tb-btn" class:on={ed.isActive('bold')}
          onclick={() => ed.chain().focus().toggleBold().run()} title="Bold (Ctrl+B)">
          <i class="bi bi-type-bold"></i>
        </button>
        <button class="tb-btn" class:on={ed.isActive('italic')}
          onclick={() => ed.chain().focus().toggleItalic().run()} title="Italic (Ctrl+I)">
          <i class="bi bi-type-italic"></i>
        </button>
        <button class="tb-btn" class:on={ed.isActive('strike')}
          onclick={() => ed.chain().focus().toggleStrike().run()} title="Strikethrough">
          <i class="bi bi-type-strikethrough"></i>
        </button>

        <div class="tb-sep"></div>

        <div class="tb-font-picker">
          <FontPicker
            value={ed.getAttributes('textStyle').fontFamily || ''}
            onchange={(font) => {
              if (font) ed.chain().focus().setFontFamily(font).run();
              else ed.chain().focus().unsetFontFamily().run();
            }}
            placeholder="Font" />
        </div>

        <select class="tb-select" title="Font size"
          value={ed.getAttributes('textStyle').fontSize || ''}
          onchange={(e) => {
            const v = e.target.value;
            if (v) ed.chain().focus().setFontSize(v).run();
            else ed.chain().focus().unsetFontSize().run();
          }}>
          <option value="">Default</option>
          {#each FONT_SIZES as size}
            <option value={size}>{size}</option>
          {/each}
        </select>

        <div class="tb-sep"></div>

        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'left' })}
          onclick={() => ed.chain().focus().setTextAlign('left').run()} title="Align left">
          <i class="bi bi-text-left"></i>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'center' })}
          onclick={() => ed.chain().focus().setTextAlign('center').run()} title="Center">
          <i class="bi bi-text-center"></i>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'right' })}
          onclick={() => ed.chain().focus().setTextAlign('right').run()} title="Align right">
          <i class="bi bi-text-right"></i>
        </button>
        <button class="tb-btn" class:on={ed.isActive({ textAlign: 'justify' })}
          onclick={() => ed.chain().focus().setTextAlign('justify').run()} title="Justify">
          <i class="bi bi-justify"></i>
        </button>

        <div class="tb-sep"></div>

        <button class="tb-btn" class:on={verticalAlign === 'top'}
          onclick={() => verticalAlign = 'top'} title="Page: align top">
          <i class="bi bi-align-top"></i>
        </button>
        <button class="tb-btn" class:on={verticalAlign === 'center'}
          onclick={() => verticalAlign = 'center'} title="Page: vertically center">
          <i class="bi bi-align-middle"></i>
        </button>
        <button class="tb-btn" class:on={verticalAlign === 'bottom'}
          onclick={() => verticalAlign = 'bottom'} title="Page: align bottom">
          <i class="bi bi-align-bottom"></i>
        </button>

        <div class="tb-sep"></div>

        <button class="tb-btn" onclick={triggerImageUpload} title="Insert image">
          <i class="bi bi-image"></i>
        </button>

        <div class="tb-sep"></div>

        <button class="tb-btn"
          onclick={() => ed.chain().focus().undo().run()} title="Undo (Ctrl+Z)">
          <i class="bi bi-arrow-counterclockwise"></i>
        </button>
        <button class="tb-btn"
          onclick={() => ed.chain().focus().redo().run()} title="Redo (Ctrl+Y)">
          <i class="bi bi-arrow-clockwise"></i>
        </button>
      </div>
      <input type="file" accept="image/*" bind:this={fileInput}
        onchange={handleFileSelected} style="display: none;" />
    {/if}

    <div class="editor-canvas">
      <div class="page-surface-wrap">
        <div class="page-surface"
          data-valign={verticalAlign}
          style="
            width: {widthIn}in;
            height: {heightIn}in;
            padding-top: {marginTop}in;
            padding-bottom: {marginBottom}in;
            padding-left: {marginInside}in;
            padding-right: {marginOutside}in;
            font-size: {fontSize}pt;
            line-height: {lineSpacing};
          "
          bind:this={element}>
        </div>
        <div class="page-bottom-marker" style="width: {widthIn}in;">Page boundary — content past this point won't fit</div>
      </div>
    </div>
  </div>
</div>

<style>
  .page-editor-backdrop {
    position: fixed; inset: 0; z-index: 2000;
    background: rgba(0, 0, 0, 0.55);
    display: flex; align-items: stretch; justify-content: center;
  }
  .page-editor-shell {
    background: var(--iwe-bg);
    width: 100%; max-width: 1200px;
    margin: 1rem;
    border-radius: var(--iwe-radius);
    display: flex; flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .editor-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
    flex-shrink: 0;
  }
  .editor-title {
    display: flex; align-items: center; gap: 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.9rem;
    color: var(--iwe-text);
  }
  .editor-role-badge {
    font-size: 0.65rem; text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    background: var(--iwe-bg-hover); color: var(--iwe-text-muted);
    padding: 2px 8px; border-radius: 8px;
  }
  .editor-header-actions { display: flex; gap: 0.5rem; }
  .editor-btn {
    padding: 0.4rem 0.9rem;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
  }
  .editor-btn:hover { background: var(--iwe-bg-hover); }
  .editor-btn.primary {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .editor-btn.primary:hover { background: #245a4f; border-color: #245a4f; }

  .editor-toolbar {
    display: flex; align-items: center; gap: 2px;
    padding: 0.4rem 0.8rem;
    border-bottom: 1px solid var(--iwe-border);
    background: var(--iwe-bg-warm);
    flex-wrap: wrap;
    flex-shrink: 0;
  }
  .tb-btn {
    border: 1px solid transparent; background: none;
    padding: 0.3rem 0.5rem;
    color: var(--iwe-text-muted);
    cursor: pointer;
    border-radius: var(--iwe-radius-sm);
    font-size: 0.85rem; line-height: 1;
    transition: all 100ms;
  }
  .tb-btn:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .tb-btn.on {
    background: var(--iwe-accent); color: #fff;
  }
  .tb-sep {
    width: 1px; height: 18px; background: var(--iwe-border);
    margin: 0 0.3rem;
  }
  .tb-select {
    border: 1px solid var(--iwe-border); background: var(--iwe-bg);
    color: var(--iwe-text);
    padding: 0.25rem 0.4rem;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    border-radius: var(--iwe-radius-sm);
    cursor: pointer;
  }
  .tb-select:focus { outline: none; border-color: var(--iwe-accent); }
  .tb-font-picker {
    width: 180px;
  }

  .editor-canvas {
    flex: 1;
    overflow: auto;
    padding: 2rem;
    background: #e8e4df;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .page-surface-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .page-surface {
    background: #fff;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.18), 0 0 0 1px rgba(0, 0, 0, 0.08);
    font-family: 'Liberation Serif', 'Georgia', serif;
    color: #222;
    /* Fixed dimensions — overflow extends visibly past the boundary so the user
       sees that content won't fit. box-sizing: border-box keeps padding inside. */
    box-sizing: border-box;
    overflow: visible;
    /* Flex column lets us position the editor block at top/center/bottom */
    display: flex;
    flex-direction: column;
  }
  .page-surface[data-valign="top"]    { justify-content: flex-start; }
  .page-surface[data-valign="center"] { justify-content: center; }
  .page-surface[data-valign="bottom"] { justify-content: flex-end; }
  .page-surface :global(.ProseMirror) {
    outline: none;
    flex: 0 0 auto; /* don't stretch — let alignment work */
    width: 100%;
  }
  /* Bottom-of-page indicator: dashed line + caption sitting just under the page. */
  .page-bottom-marker {
    margin-top: 2px;
    border-top: 1px dashed #c0392b;
    text-align: center;
    font-family: var(--iwe-font-ui);
    font-size: 0.65rem;
    color: #c0392b;
    padding-top: 4px;
    pointer-events: none;
  }
  .page-surface :global(.ProseMirror p) {
    margin: 0 0 0.4em 0;
  }
  .page-surface :global(.image-outer) {
    text-align: center;
    margin: 0.5em 0;
    line-height: 0;
  }
  .page-surface :global(.image-frame) {
    display: inline-block;
    position: relative;
    line-height: 0;
  }
  .page-surface :global(.image-frame img) {
    max-width: 100%;
    height: auto;
    display: block;
  }
  .page-surface :global(.image-frame:hover .image-resize-handle),
  .page-surface :global(.image-frame.is-selected .image-resize-handle) {
    opacity: 1;
  }
  .page-surface :global(.image-frame.is-selected img) {
    outline: 2px solid var(--iwe-accent);
    outline-offset: 1px;
  }
  .page-surface :global(.image-resize-handle) {
    position: absolute;
    bottom: -6px;
    right: -6px;
    width: 14px;
    height: 14px;
    background: var(--iwe-accent);
    border: 2px solid #fff;
    border-radius: 3px;
    cursor: nwse-resize;
    opacity: 0;
    transition: opacity 100ms;
    box-shadow: 0 1px 4px rgba(0,0,0,0.25);
    z-index: 10;
    display: block;
  }
  .page-surface :global(.ProseMirror p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    color: #999;
    pointer-events: none;
    float: left;
    height: 0;
  }
</style>
