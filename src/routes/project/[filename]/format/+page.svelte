<script>
  import { onMount, onDestroy } from 'svelte';
  import { afterNavigate } from '$app/navigation';
  import { flip } from 'svelte/animate';
  import { dndzone } from 'svelte-dnd-action';
  import {
    getChapters, getFormatProfiles, getFormatPages, getSettings, saveSettings,
    seedFormatProfiles, addFormatProfile, updateFormatProfile, deleteFormatProfile,
    duplicateFormatProfile, pasteFormatProfileSettings, addFormatPage,
    updateFormatPage, deleteFormatPage, reorderFormatPages,
    addPageExclusion, removePageExclusion, listPageExclusions,
    compilePreview, exportFormatPdf, exportEpub, validateEpubBytes,
    getProjectSetting, setProjectSetting,
    updateProfileCategory,
    getBookCover,
  } from '$lib/db.js';
  import { createChapterDoc, destroyDoc } from '$lib/ydoc.js';
  import { yDocToProsemirrorJSON } from 'y-prosemirror';
  import { generateHTML, Node, mergeAttributes } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import TextAlign from '@tiptap/extension-text-align';
  import { TextStyle, FontSize, FontFamily } from '@tiptap/extension-text-style';
  import Superscript from '@tiptap/extension-superscript';
  import Subscript from '@tiptap/extension-subscript';
  import { NoteMarker } from '$lib/noteMarker.js';
  import { StateMarker } from '$lib/stateMarker.js';
  import { TimeBreak } from '$lib/timeBreak.js';
  import { save } from '@tauri-apps/plugin-dialog';
  import { addToast } from '$lib/toast.js';
  import PageContentEditor from '$lib/components/PageContentEditor.svelte';
  import TocPageEditor from '$lib/components/TocPageEditor.svelte';
  import MapPageEditor from '$lib/components/MapPageEditor.svelte';
  import ChapterHeadings from '$lib/components/format/ChapterHeadings.svelte';
  import ParagraphSettings from '$lib/components/format/ParagraphSettings.svelte';
  import HeadingsSettings from '$lib/components/format/HeadingsSettings.svelte';
  import BreaksSettings from '$lib/components/format/BreaksSettings.svelte';
  import PrintLayoutSettings from '$lib/components/format/PrintLayoutSettings.svelte';
  import TypographySettings from '$lib/components/format/TypographySettings.svelte';
  import HeaderFooterSettings from '$lib/components/format/HeaderFooterSettings.svelte';
  import TrimSettings from '$lib/components/format/TrimSettings.svelte';

  const flipDurationMs = 150;


  // Page types determine which editor/renderer a page uses.
  // Extensible — add new types here as new page kinds are built (e.g. 'map').
  const PAGE_TYPES = [
    { id: 'free-form', label: 'Free Form', icon: 'bi-card-text', description: 'Rich text page with full editor' },
    { id: 'toc',       label: 'Table of Contents', icon: 'bi-list-ol', description: 'Auto-generated from chapters' },
    { id: 'map',       label: 'Map', icon: 'bi-map', description: 'Full-page image, single or double spread' },
  ];

  function pageTypeLabel(role) {
    const found = PAGE_TYPES.find(t => t.id === role);
    return found ? found.label : role;
  }

  // Ebook metadata tags — semantic roles for EPUB export (epub:type).
  // These annotate pages with their semantic purpose in an ebook.
  const EBOOK_TAGS = [
    'half-title', 'title', 'copyright', 'dedication', 'epigraph', 'toc',
    'foreword', 'preface', 'prologue', 'epilogue', 'afterword',
    'acknowledgments', 'about-author', 'also-by', 'glossary', 'excerpt',
    'blurbs',
  ];

  function ebookTagLabel(tag) {
    const labels = {
      'half-title': 'Half Title', 'title': 'Title Page', 'copyright': 'Copyright',
      'dedication': 'Dedication', 'epigraph': 'Epigraph', 'toc': 'Table of Contents',
      'foreword': 'Foreword', 'preface': 'Preface', 'prologue': 'Prologue',
      'epilogue': 'Epilogue', 'afterword': 'Afterword', 'acknowledgments': 'Acknowledgments',
      'about-author': 'About the Author', 'also-by': 'Also By', 'glossary': 'Glossary',
      'excerpt': 'Excerpt', 'blurbs': 'Blurbs',
    };
    return labels[tag] || tag;
  }

  // Custom mode sub-tabs (each is a settings component)
  const ALL_CUSTOM_TABS = [
    { id: 'chapter-headings', label: 'Chapter Headings', icon: 'bi-bookmark', targets: ['print', 'ebook'] },
    { id: 'paragraph',        label: 'Paragraph',        icon: 'bi-paragraph', targets: ['print', 'ebook'] },
    { id: 'headings',         label: 'Headings',         icon: 'bi-type-h1', targets: ['print', 'ebook'] },
    { id: 'breaks',           label: 'Breaks',           icon: 'bi-asterisk', targets: ['print', 'ebook'] },
    { id: 'print-layout',     label: 'Print Layout',     icon: 'bi-layout-text-window', targets: ['print'] },
    { id: 'typography',       label: 'Typography',       icon: 'bi-fonts', targets: ['print', 'ebook'] },
    { id: 'header-footer',    label: 'Header / Footer',  icon: 'bi-distribute-vertical', targets: ['print'] },
    { id: 'trim',             label: 'Target Format',    icon: 'bi-aspect-ratio', targets: ['print', 'ebook'] },
  ];
  let customTab = $state('chapter-headings');
  let customSelectorOpen = $state(false);
  let filteredCustomTabs = $derived(
    ALL_CUSTOM_TABS.filter(t => t.targets.includes(activeProfile?.target_type || 'print'))
  );
  let activeCustomTab = $derived(filteredCustomTabs.find(t => t.id === customTab) || filteredCustomTabs[0]);

  function selectCustomTab(id) {
    customTab = id;
    customSelectorOpen = false;
  }

  // Sidebar modes
  const SIDEBAR_MODES = [
    { key: 'pages', label: 'Pages', icon: 'bi-file-earmark-text' },
    { key: 'themes', label: 'Themes', icon: 'bi-palette' },
    { key: 'custom', label: 'Custom', icon: 'bi-sliders' },
    { key: 'export', label: 'Export', icon: 'bi-download' },
  ];

  // Resize
  let sidebarWidth = $state(320);
  let dragging = $state(false);
  let saveWidthTimer = null;
  const SIDEBAR_MIN = 220;
  const SIDEBAR_MAX = 600;

  function persistWidth() {
    clearTimeout(saveWidthTimer);
    saveWidthTimer = setTimeout(async () => {
      const settings = await getSettings();
      settings.formatSidebarWidth = sidebarWidth;
      await saveSettings(settings);
    }, 300);
  }

  function startDrag() {
    dragging = true;
    const onMove = (e) => {
      if (!dragging) return;
      const newWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, window.innerWidth - e.clientX));
      sidebarWidth = newWidth;
    };
    const onUp = () => {
      dragging = false;
      persistWidth();
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  // State
  let loading = $state(true);
  let profiles = $state([]);
  let activeProfileId = $state(null);
  let sidebarMode = $state('pages');
  let chapters = $state([]);
  let formatPages = $state([]); // ALL project-level pages
  let exclusions = $state([]); // [{ page_id, profile_id }]

  // Profile management UI state
  let showProfileMenu = $state(false);
  let showCreateProfileModal = $state(false);
  let newProfileName = $state('');
  let newProfileDuplicateFrom = $state(null); // profile id or null
  let newProfileTargetType = $state('print');
  let renamingProfileId = $state(null);
  let renameValue = $state('');
  let confirmDeleteProfileId = $state(null);

  // Clipboard for copy/paste profile settings (persists in settings.json)
  let formatClipboard = $state(null); // raw profile object snapshot, or null

  // Helper: is a page included in a given profile?
  function isPageIncludedIn(pageId, profileId) {
    return !exclusions.some(e => e.page_id === pageId && e.profile_id === profileId);
  }

  // Ebook metadata tag bar state — only visible for ebook profiles
  let armedTag = $state(null);
  let usedEbookTags = $derived(new Map(
    formatPages.filter(p => p.ebook_metadata_tag).map(p => [p.ebook_metadata_tag, p.id])
  ));

  // Add-page modal state
  let showAddPageModal = $state(false);
  let addPagePosition = $state('front'); // 'front' or 'back'

  // Inline rename state for format pages
  let editingPageId = $state(null);
  let editingPageTitle = $state('');

  // Full content editor modal state
  let designingPage = $state(null); // FormatPage being edited (free-form), or null
  let editingTocPage = $state(null); // FormatPage being edited (toc), or null
  let editingMapPage = $state(null); // FormatPage being edited (map), or null

  // Re-attach observer whenever the page count or compile generation changes
  $effect(() => {
    // Reactive deps
    pageCount;
    compileGeneration;
    // Wait for DOM to render new placeholders before observing
    queueMicrotask(() => setupObserver());
  });

  // Compiled preview state
  let pageCount = $state(0);
  let rendering = $state(false); // true during compile
  let renderError = $state(null);
  let lastTiming = $state(null); // CompileTiming from Rust
  let compileGeneration = $state(0); // incremented on each compile to bust img cache

  // Export state
  let exporting = $state(false);
  let exportError = $state(null);
  let sizeEstimate = $state(null); // { total, breakdown, imageCount } or null
  let estimating = $state(false);
  // Image compression preset for EPUB export: 'none' | 'balanced' | 'compact'
  let compressionLevel = $state('none');

  // Ebook preview
  let ebookPreviewHtml = $state('');
  let ebookDevice = $state('kindle-paperwhite');

  // Device dimensions lookup
  const DEVICE_DIMS = {
    'kindle-paperwhite': { w: 300, h: 480 },
    'kindle-oasis': { w: 320, h: 500 },
    'ipad-mini': { w: 380, h: 540 },
    'ipad': { w: 420, h: 600 },
    'ipad-pro': { w: 460, h: 660 },
    'iphone': { w: 260, h: 480 },
    'iphone-max': { w: 280, h: 520 },
    'android-phone': { w: 270, h: 500 },
    'android-tablet': { w: 400, h: 580 },
    'kobo-libra': { w: 310, h: 490 },
  };
  let deviceDims = $derived(DEVICE_DIMS[ebookDevice] || { w: 300, h: 480 });
  let sectionPages = $state({}); // section_id -> 0-based page index

  // Lazy-load state — IntersectionObserver tracks visible pages, scroll-idle commits loads
  let loadedPages = $state(new Set()); // page indices that have had src set
  let visibleSet = new Set(); // currently intersecting (not reactive — used by handlers only)
  let pageObserver = null;
  let scrollIdleTimer = null;
  let scrolling = false;
  const SCROLL_IDLE_MS = 150;
  const VISIBLE_BUFFER = 2; // load 2 pages above and below the visible range

  // DnD items for front and back sections
  let frontItems = $state([]);
  let backItems = $state([]);

  $effect(() => {
    frontItems = formatPages
      .filter(p => p.position === 'front')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, dndId: p.id }));
  });

  $effect(() => {
    backItems = formatPages
      .filter(p => p.position === 'back')
      .sort((a, b) => a.sort_order - b.sort_order)
      .map(p => ({ ...p, dndId: p.id }));
  });

  let activeProfile = $derived(profiles.find(p => p.id === activeProfileId) || null);
  let isEbook = $derived(activeProfile?.target_type === 'ebook');

  // Recompute the export size estimate when the user switches to the Export
  // sidebar tab on an ebook profile, changes compression level, or content
  // changes. Skipped for print profiles and non-export modes since it's
  // only shown on the Export panel for ebooks.
  $effect(() => {
    // Track reactive dependencies explicitly
    const _mode = sidebarMode;
    const _ebook = isEbook;
    const _chapters = chapters;
    const _pages = formatPages;
    const _profile = activeProfile;
    const _level = compressionLevel;

    if (_mode !== 'export' || !_ebook || !_profile) {
      sizeEstimate = null;
      return;
    }

    estimating = true;
    estimateExportSize(_level)
      .then(r => { sizeEstimate = r; })
      .catch(e => {
        console.error('[format] estimateExportSize failed', e);
        sizeEstimate = null;
      })
      .finally(() => { estimating = false; });
  });

  // Preview scroll ref
  let previewContainer;

  // ---- Lazy loading ----

  function teardownObserver() {
    if (pageObserver) {
      pageObserver.disconnect();
      pageObserver = null;
    }
    visibleSet.clear();
  }

  function setupObserver() {
    teardownObserver();
    if (!previewContainer || pageCount === 0) return;

    pageObserver = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const idx = Number(entry.target.dataset.pageIndex);
        if (entry.isIntersecting) {
          visibleSet.add(idx);
        } else {
          visibleSet.delete(idx);
        }
      }
      // If not currently scrolling, commit immediately (initial load case)
      if (!scrolling) {
        scheduleCommit();
      }
    }, {
      root: previewContainer,
      // Use a large rootMargin so pages just outside viewport count as "visible"
      // and we get a smooth experience without waiting for them to fully enter
      rootMargin: '200px 0px 200px 0px',
      threshold: 0,
    });

    // Observe all current page placeholders
    const elements = previewContainer.querySelectorAll('[data-page-index]');
    for (const el of elements) {
      pageObserver.observe(el);
    }
  }

  function scheduleCommit() {
    clearTimeout(scrollIdleTimer);
    scrollIdleTimer = setTimeout(commitVisible, SCROLL_IDLE_MS);
  }

  function commitVisible() {
    scrolling = false;
    if (visibleSet.size === 0) return;

    // Compute the range we want loaded: visible pages + buffer
    const sorted = [...visibleSet].sort((a, b) => a - b);
    const min = Math.max(0, sorted[0] - VISIBLE_BUFFER);
    const max = Math.min(pageCount - 1, sorted[sorted.length - 1] + VISIBLE_BUFFER);

    const updated = new Set(loadedPages);
    let changed = false;
    for (let i = min; i <= max; i++) {
      if (!updated.has(i)) {
        updated.add(i);
        changed = true;
      }
    }
    if (changed) {
      loadedPages = updated;
    }

    // Persist current scroll position for the active profile so a recompile
    // (settings change) can restore the user's exact viewport.
    persistScrollPosition();
  }

  // ---- Scroll position persistence ----
  let scrollSaveTimer = null;
  function persistScrollPosition() {
    if (!activeProfileId || !previewContainer) return;
    clearTimeout(scrollSaveTimer);
    const top = previewContainer.scrollTop;
    scrollSaveTimer = setTimeout(() => {
      setProjectSetting(`format_scroll_${activeProfileId}`, String(top)).catch(() => {});
    }, 200);
  }

  async function loadSavedScroll(profileId) {
    try {
      const v = await getProjectSetting(`format_scroll_${profileId}`);
      const n = Number(v);
      return Number.isFinite(n) && n >= 0 ? n : 0;
    } catch {
      return 0;
    }
  }

  /// Restore scroll position synchronously (no animation) after a render cycle.
  /// Called from compileAndShow once placeholders are in the DOM.
  function restoreScroll(top) {
    if (!previewContainer || top == null) return;
    previewContainer.scrollTop = top;
  }

  function handlePreviewScroll() {
    scrolling = true;
    scheduleCommit();
  }

  // ---- Chapter heading rendering (ebook preview) ----
  // Keep these defaults in sync with ChapterHeadings.svelte's defaults() —
  // they're the baseline used when a profile's chapter_headings_json is empty
  // or missing keys.
  const CHAPTER_HEADING_DEFAULTS = {
    number_enabled: true,
    number_format: 'chapter_numeric',
    number_font: '',
    number_size_pt: 14,
    number_align: 'center',
    number_style: 'uppercase',
    number_tracking_em: 0,
    title_enabled: true,
    title_font: '',
    title_size_pt: 18,
    title_align: 'center',
    title_style: 'regular',
    title_tracking_em: 0,
    subtitle_enabled: true,
    subtitle_font: '',
    subtitle_size_pt: 12,
    subtitle_align: 'center',
    subtitle_style: 'italic',
    subtitle_tracking_em: 0,
    space_number_title_em: 1.5,
    space_title_subtitle_em: 0.8,
    space_after_heading_em: 3,
    rule_above: false,
    rule_below: false,
    rule_thickness_pt: 0.5,
  };

  const NUMBER_WORDS_ONES = [
    'Zero','One','Two','Three','Four','Five','Six','Seven','Eight','Nine',
    'Ten','Eleven','Twelve','Thirteen','Fourteen','Fifteen','Sixteen','Seventeen','Eighteen','Nineteen',
  ];
  const NUMBER_WORDS_TENS = ['', '', 'Twenty', 'Thirty', 'Forty', 'Fifty', 'Sixty', 'Seventy', 'Eighty', 'Ninety'];

  function numberToWords(n) {
    if (n < 20) return NUMBER_WORDS_ONES[n];
    if (n < 100) {
      const tens = Math.floor(n / 10);
      const ones = n % 10;
      return NUMBER_WORDS_TENS[tens] + (ones ? '-' + NUMBER_WORDS_ONES[ones].toLowerCase() : '');
    }
    if (n < 1000) {
      const hundreds = Math.floor(n / 100);
      const rem = n % 100;
      return NUMBER_WORDS_ONES[hundreds] + ' Hundred' + (rem ? ' ' + numberToWords(rem) : '');
    }
    return String(n);
  }

  function numberToRoman(n) {
    const pairs = [
      [1000, 'M'], [900, 'CM'], [500, 'D'], [400, 'CD'],
      [100, 'C'], [90, 'XC'], [50, 'L'], [40, 'XL'],
      [10, 'X'], [9, 'IX'], [5, 'V'], [4, 'IV'], [1, 'I'],
    ];
    let out = '';
    for (const [v, s] of pairs) {
      while (n >= v) { out += s; n -= v; }
    }
    return out;
  }

  function formatChapterNumber(n, format) {
    switch (format) {
      case 'none':           return '';
      case 'numeric':        return String(n);
      case 'chapter_numeric':return `Chapter ${n}`;
      case 'word':           return numberToWords(n);
      case 'chapter_word':   return `Chapter ${numberToWords(n)}`;
      case 'roman':          return numberToRoman(n);
      case 'chapter_roman':  return `Chapter ${numberToRoman(n)}`;
      default:               return `Chapter ${n}`;
    }
  }

  // Build the inline style string for one of the three heading elements
  // (number, title, subtitle). Reads {kind}_font, {kind}_size_pt, {kind}_align,
  // {kind}_style, {kind}_tracking_em from the resolved settings object.
  function headingElementStyle(h, kind) {
    const font = h[`${kind}_font`];
    const size = h[`${kind}_size_pt`];
    const align = h[`${kind}_align`];
    const style = h[`${kind}_style`];
    const tracking = h[`${kind}_tracking_em`] || 0;

    let css = 'margin:0;text-indent:0;';
    if (align) css += `text-align:${align};`;
    if (size) css += `font-size:${size}pt;`;
    if (font) css += `font-family:"${font}",serif;`;
    if (tracking) css += `letter-spacing:${tracking}em;`;

    switch (style) {
      case 'bold':      css += 'font-weight:700;'; break;
      case 'italic':    css += 'font-style:italic;font-weight:400;'; break;
      case 'smallcaps': css += 'font-variant:small-caps;font-weight:400;'; break;
      case 'uppercase': css += 'text-transform:uppercase;font-weight:400;'; break;
      default:          css += 'font-weight:400;'; break;
    }
    return css;
  }

  // Resolve the chapter image src for a given chapter + chapter-heading settings.
  // Returns a data URL / src string, or null if no image should be shown.
  //   - image_enabled gates everything
  //   - image_individual=true: prefer ch.chapter_image, fall back to profile default
  //   - image_individual=false: always use profile default
  function resolveChapterImageSrc(chapter, h) {
    if (!h.image_enabled) return null;
    if (h.image_individual) {
      return (chapter.chapter_image && chapter.chapter_image.trim())
        ? chapter.chapter_image
        : (h.image_default || null);
    }
    return h.image_default || null;
  }

  function buildChapterImageHtml(src, widthPct, align) {
    if (!src) return '';
    let marginClause;
    if (align === 'left')  marginClause = 'margin:0.5em auto 0.5em 0;';
    else if (align === 'right') marginClause = 'margin:0.5em 0 0.5em auto;';
    else marginClause = 'margin:0.5em auto;';
    const w = Math.max(1, Math.min(150, Number(widthPct) || 50));
    return `<img src="${src}" alt="" style="display:block;width:${w}%;max-width:100%;${marginClause}">`;
  }

  function renderChapterHeadingHtml(chapter, index, h) {
    const chNum = index + 1;

    const imgSrc = resolveChapterImageSrc(chapter, h);
    const imgPosition = imgSrc ? h.image_position : null;
    const imgHtml = buildChapterImageHtml(imgSrc, h.image_width_pct, h.image_align);

    // Build the three text lines (number, title, subtitle) once — they're used
    // both for the normal layout and for the cover_heading overlay.
    const showNumber = h.number_enabled && h.number_format !== 'none';
    const numberText = showNumber ? formatChapterNumber(chNum, h.number_format) : '';
    const numberHtml = (showNumber && numberText)
      ? `<p style="${headingElementStyle(h, 'number')}">${escapeHtml(numberText)}</p>`
      : '';

    const showTitle = h.title_enabled && chapter.title;
    const titleHtml = showTitle
      ? `<p style="${headingElementStyle(h, 'title')}${numberHtml ? `margin-top:${h.space_number_title_em}em;` : ''}">${escapeHtml(chapter.title)}</p>`
      : '';

    const showSubtitle = h.subtitle_enabled && chapter.subtitle;
    const subtitleHtml = showSubtitle
      ? `<p style="${headingElementStyle(h, 'subtitle')}${(titleHtml || numberHtml) ? `margin-top:${h.space_title_subtitle_em}em;` : ''}">${escapeHtml(chapter.subtitle)}</p>`
      : '';

    const ruleAboveHtml = h.rule_above
      ? `<div style="border-top:${h.rule_thickness_pt}pt solid currentColor;margin:0 0 0.75em 0;"></div>`
      : '';
    const ruleBelowHtml = h.rule_below
      ? `<div style="border-top:${h.rule_thickness_pt}pt solid currentColor;margin:0.75em 0 0 0;"></div>`
      : '';

    const wrapperStyle = `margin:2em 0 ${h.space_after_heading_em}em 0;`;

    // Special case: cover_heading wraps the three text lines over a background
    // image. The image itself isn't emitted as a separate <img> — it becomes
    // the backdrop. image_light_text switches the text color to white.
    //
    // IMPORTANT: this goes into an inline `style="..."` attribute, so the CSS
    // inside must not use double quotes (they'd close the attribute early).
    // Use single quotes for the url() argument.
    if (imgPosition === 'cover_heading') {
      const textColor = h.image_light_text ? '#ffffff' : 'inherit';
      const coverInner = numberHtml + titleHtml + subtitleHtml;
      const safeSrc = String(imgSrc).replace(/'/g, "%27");
      const coverStyle =
        `background:url('${safeSrc}') center/cover no-repeat;` +
        `padding:4em 1.5em;min-height:180px;color:${textColor};`;
      return `<div class="ebook-chapter-heading" style="${wrapperStyle}">${ruleAboveHtml}<div style="${coverStyle}">${coverInner}</div>${ruleBelowHtml}</div>`;
    }

    // Special case: dedicated_page puts the image in its own standalone section
    // BEFORE the chapter heading. In EPUB readers this becomes its own screen
    // via page-break-after, and the chapter heading follows on the next screen.
    //
    // The wrapper has a FIXED height (not min-height) so the image's center
    // point stays anchored as the user scales image_width_pct — otherwise the
    // wrapper grows with the image and the center drifts downward.
    // Using a fixed height also makes `max-height: 100%` on the img resolve
    // correctly in flex layout.
    let dedicatedPageHtml = '';
    if (imgPosition === 'dedicated_page' && imgSrc) {
      const w = Math.max(1, Math.min(100, Number(h.image_width_pct) || 80));
      dedicatedPageHtml =
        `<div class="ebook-dedicated-image-page" ` +
        `style="page-break-before:always;page-break-after:always;` +
        `break-before:page;break-after:page;` +
        `display:flex;align-items:center;justify-content:center;` +
        `height:400px;margin:3em 0;overflow:hidden;">` +
        `<img src="${imgSrc}" alt="" ` +
        `style="width:${w}%;max-width:100%;max-height:100%;height:auto;object-fit:contain;">` +
        `</div>`;
    }

    // Normal layout: assemble image + text parts by position.
    const parts = [];
    parts.push(ruleAboveHtml);
    if (imgPosition === 'above_number') parts.push(imgHtml);
    parts.push(numberHtml);
    if (imgPosition === 'between_number_title') parts.push(imgHtml);
    parts.push(titleHtml);
    if (imgPosition === 'between_title_subtitle') parts.push(imgHtml);
    parts.push(subtitleHtml);
    if (imgPosition === 'below_heading') parts.push(imgHtml);
    parts.push(ruleBelowHtml);

    return dedicatedPageHtml + `<div class="ebook-chapter-heading" style="${wrapperStyle}">${parts.join('')}</div>`;
  }

  // ---- Defaults for the other Custom subpanels. Keep in sync with
  // the defaults() functions in each .svelte component.

  const PARAGRAPH_DEFAULTS = {
    drop_cap_enabled: false,
    drop_cap_lines: 2,
    drop_cap_font: '',
    drop_cap_color: '#000000',
    drop_cap_quote_mode: 'letter_only',
    small_caps_enabled: false,
    small_caps_words: 5,
    apply_when: 'chapter',
    paragraph_style: 'indented',
    indent_em: 1.5,
    spacing_em: 0.5,
    prevent_widows: true,
    prevent_orphans: true,
  };

  const HEADINGS_DEFAULTS = {
    no_indent_after: true,
    h2_enabled: true, h2_font: '', h2_size_pt: 16, h2_align: 'left', h2_style: 'bold',
    h2_tracking_em: 0, h2_space_above_em: 1.5, h2_space_below_em: 0.8,
    h2_rule_above: false, h2_rule_below: false,
    h3_enabled: true, h3_font: '', h3_size_pt: 13, h3_align: 'left', h3_style: 'bold',
    h3_tracking_em: 0, h3_space_above_em: 1.2, h3_space_below_em: 0.6,
    h3_rule_above: false, h3_rule_below: false,
    h4_enabled: true, h4_font: '', h4_size_pt: 11, h4_align: 'left', h4_style: 'italic',
    h4_tracking_em: 0, h4_space_above_em: 1.0, h4_space_below_em: 0.4,
    h4_rule_above: false, h4_rule_below: false,
  };

  const BREAKS_DEFAULTS = {
    style: 'dinkus',
    custom_text: '* * *',
    space_above_em: 1.2,
    space_below_em: 1.2,
    image_data: '',
    image_width_pct: 25,
  };

  // Emit the font-weight/style/variant/transform CSS clauses for a style enum.
  // Shared between chapter heading renderer and h2/h3/h4 CSS builder.
  function styleClauseFor(style) {
    switch (style) {
      case 'bold':      return 'font-weight:700;';
      case 'italic':    return 'font-style:italic;font-weight:400;';
      case 'smallcaps': return 'font-variant:small-caps;font-weight:400;';
      case 'uppercase': return 'text-transform:uppercase;font-weight:400;';
      default:          return 'font-weight:400;';
    }
  }

  function parseProfileJson(raw, defaults) {
    if (!raw) return { ...defaults };
    try {
      return { ...defaults, ...JSON.parse(raw) };
    } catch {
      return { ...defaults };
    }
  }

  function buildParagraphCss(p) {
    const useIndent = p.paragraph_style === 'indented' || p.paragraph_style === 'both';
    const useSpacing = p.paragraph_style === 'spaced' || p.paragraph_style === 'both';
    const indent = useIndent ? `${p.indent_em}em` : '0';
    const spacing = useSpacing ? `${p.spacing_em}em` : '0';

    const rules = [
      `.ebook-body p { margin: 0 0 ${spacing}; text-indent: ${indent}; }`,
      // Empty paragraphs (blank lines from pressing Enter) collapse to zero
      // height in HTML. Inject a non-breaking space so they occupy one line.
      `.ebook-body p:empty::before { content: '\\00a0'; }`,
      // First paragraph after a chapter heading or scene break loses its indent.
      `.ebook-body .ebook-chapter-heading + p { text-indent: 0; }`,
      `.ebook-body hr + p { text-indent: 0; }`,
    ];

    // Drop cap — CSS ::first-letter approximation. Quote mode nuances
    // (first_char / both_together / letter_only / disable_on_dialogue) are
    // not expressible in pure CSS — the preview shows default ::first-letter
    // behavior which roughly matches "both_together".
    if (p.drop_cap_enabled) {
      const lines = p.drop_cap_lines || 2;
      const size = lines * 1.2;
      const fontClause = p.drop_cap_font ? `font-family:"${p.drop_cap_font}",serif;` : '';
      const colorClause = p.drop_cap_color ? `color:${p.drop_cap_color};` : '';

      const selectors = [];
      if (p.apply_when === 'chapter' || p.apply_when === 'both') {
        selectors.push('.ebook-body .ebook-chapter-heading + p::first-letter');
      }
      if (p.apply_when === 'breaks' || p.apply_when === 'both') {
        selectors.push('.ebook-body hr + p::first-letter');
      }
      if (selectors.length) {
        rules.push(`${selectors.join(', ')} {
          float: left;
          font-size: ${size}em;
          line-height: 0.9;
          padding: 0.08em 0.08em 0 0;
          ${fontClause}${colorClause}
        }`);
      }
    }

    // Small caps — only the `first line` case (small_caps_words === -1) is
    // expressible with CSS ::first-line. Specific word counts would require
    // span-wrapping the HTML, which isn't done here.
    if (p.small_caps_enabled && p.small_caps_words === -1) {
      const selectors = [];
      if (p.apply_when === 'chapter' || p.apply_when === 'both') {
        selectors.push('.ebook-body .ebook-chapter-heading + p::first-line');
      }
      if (p.apply_when === 'breaks' || p.apply_when === 'both') {
        selectors.push('.ebook-body hr + p::first-line');
      }
      if (selectors.length) {
        rules.push(`${selectors.join(', ')} { font-variant: small-caps; }`);
      }
    }

    return rules.join('\n        ');
  }

  function buildHeadingsCss(h) {
    const rules = [];
    for (const level of ['h2', 'h3', 'h4']) {
      if (!h[`${level}_enabled`]) {
        rules.push(`.ebook-body ${level} { display: none; }`);
        continue;
      }
      const decl = [];
      decl.push(`margin: ${h[`${level}_space_above_em`]}em 0 ${h[`${level}_space_below_em`]}em 0`);
      if (h[`${level}_font`])        decl.push(`font-family: "${h[`${level}_font`]}", serif`);
      if (h[`${level}_size_pt`])     decl.push(`font-size: ${h[`${level}_size_pt`]}pt`);
      if (h[`${level}_align`])       decl.push(`text-align: ${h[`${level}_align`]}`);
      if (h[`${level}_tracking_em`]) decl.push(`letter-spacing: ${h[`${level}_tracking_em`]}em`);
      const styleClause = styleClauseFor(h[`${level}_style`]);
      rules.push(`.ebook-body ${level} { ${decl.join('; ')}; ${styleClause} }`);
      if (h[`${level}_rule_above`]) {
        rules.push(`.ebook-body ${level} { border-top: 1px solid currentColor; padding-top: 0.35em; }`);
      }
      if (h[`${level}_rule_below`]) {
        rules.push(`.ebook-body ${level} { border-bottom: 1px solid currentColor; padding-bottom: 0.2em; }`);
      }
    }
    if (h.no_indent_after) {
      rules.push(`.ebook-body h2 + p, .ebook-body h3 + p, .ebook-body h4 + p { text-indent: 0; }`);
    }
    return rules.join('\n        ');
  }

  function buildBreaksCss(b) {
    const mTop = b.space_above_em || 0;
    const mBot = b.space_below_em || 0;
    const rules = [];

    // Reset hr. Key points:
    //   - `opacity: 1` overrides Bootstrap's global `hr { opacity: .25 }`
    //     which otherwise fades every scene break.
    //   - No `height: 0` or `overflow: visible` — those forced ::after content
    //     to overflow outside the hr's layout box, so image breaks couldn't
    //     grow the hr's height and would overlap neighbouring content.
    //     Letting the hr auto-size to its ::after content fixes both issues.
    rules.push(`.ebook-body hr {
          border: none;
          display: block;
          text-align: center;
          line-height: 1;
          margin: ${mTop}em 0 ${mBot}em 0;
          color: #666;
          letter-spacing: 0.3em;
          opacity: 1;
          background: none;
        }`);

    switch (b.style) {
      case 'none':
        rules.push(`.ebook-body hr { display: none; }`);
        break;
      case 'blank':
        // Margin alone provides the visual gap; no glyph.
        break;
      case 'dinkus':
        rules.push(`.ebook-body hr::after { content: "* * *"; }`);
        break;
      case 'asterism':
        rules.push(`.ebook-body hr::after { content: "⁂"; font-size: 1.3em; letter-spacing: 0; }`);
        break;
      case 'rule':
        rules.push(`.ebook-body hr { border-top: 1px solid currentColor; width: 30%; margin-left: auto; margin-right: auto; }`);
        break;
      case 'custom': {
        const txt = String(b.custom_text || '* * *').replace(/\\/g, '\\\\').replace(/"/g, '\\"');
        rules.push(`.ebook-body hr::after { content: "${txt}"; }`);
        break;
      }
      case 'image':
        // Image breaks are NOT handled via CSS ::after on hr — browser
        // sizing behavior on replaced-content pseudo-elements is too
        // inconsistent to give reliable percentage widths across arbitrary
        // image aspect ratios. Instead, generateEbookPreview post-processes
        // the final HTML and replaces every <hr> with a real <img> wrapper.
        // See substituteImageBreaks below.
        rules.push(`.ebook-body hr { display: none; }`);
        break;
    }

    return rules.join('\n        ');
  }

  // Resolve all five Custom category JSON blobs for a profile with defaults
  // merged in. Shared by the preview and the EPUB export.
  function resolveEbookSettings(profile) {
    let typo = {};
    try { typo = JSON.parse(profile?.typography_json || '{}'); } catch {}
    return {
      bodyFont: typo.font || profile?.font_body || 'Georgia',
      fontSize: typo.size_pt || profile?.font_size_pt || 11,
      lineSpacing: typo.line_spacing || profile?.line_spacing || 1.4,
      chHeadings: parseProfileJson(profile?.chapter_headings_json, CHAPTER_HEADING_DEFAULTS),
      paragraph: parseProfileJson(profile?.paragraph_json, PARAGRAPH_DEFAULTS),
      headings: parseProfileJson(profile?.headings_json, HEADINGS_DEFAULTS),
      breaks: parseProfileJson(profile?.breaks_json, BREAKS_DEFAULTS),
    };
  }

  // Build the ebook CSS string. Used by both the preview ({ inline: true }
  // wraps in a <style> tag and includes preview-only scaffolding) and the
  // export ({ inline: false } returns a raw CSS string that becomes the
  // EPUB's stylesheet.css).
  function buildEbookCss(settings, opts = {}) {
    const { bodyFont, fontSize, lineSpacing, paragraph, headings, breaks } = settings;
    const inline = opts.inline ?? true;

    const widowsOrphans = [
      paragraph.prevent_widows ? 'widows: 3;' : '',
      paragraph.prevent_orphans ? 'orphans: 3;' : '',
    ].filter(Boolean).join(' ');

    const previewOnly = inline ? `
        /* Preview-only decoration between sections */
        .ebook-chapter-break {
          border-top: 1px solid #e0e0e0;
          margin: 3em 0 0;
          padding-top: 0;
        }` : '';

    const bodyPadding = inline ? 'padding: 1.5rem;' : '';

    const css = `
        .ebook-body {
          font-family: "${bodyFont}", serif;
          font-size: ${fontSize}pt;
          line-height: ${lineSpacing};
          color: #222;
          ${bodyPadding}
          ${widowsOrphans}
        }
        /* Front/back matter page titles */
        .ebook-body h1 {
          text-align: center; font-size: 1.5em; font-weight: 400;
          margin: 2em 0 1em; font-family: "${bodyFont}", serif;
        }
        .ebook-body h1 + p { text-indent: 0; }
        /* Paragraph category */
        ${buildParagraphCss(paragraph)}
        /* Headings category (h2/h3/h4) */
        ${buildHeadingsCss(headings)}
        /* Breaks category (scene break hr) */
        ${buildBreaksCss(breaks)}
        /* Auto-generated TOC */
        .ebook-body .ebook-toc h2 {
          text-align: center; font-size: 1.3em; font-weight: 400;
          margin: 1em 0 0.8em; font-family: "${bodyFont}", serif;
        }
        .ebook-body .ebook-toc ol {
          list-style: none; padding: 0; margin: 0;
        }
        .ebook-body .ebook-toc li {
          padding: 0.35em 0;
          border-bottom: 1px solid rgba(0,0,0,0.06);
          font-size: 1em;
        }
        .ebook-body .ebook-toc li:last-child { border-bottom: none; }
        .ebook-body .ebook-toc a {
          color: inherit; text-decoration: none;
        }
        /* Custom pages — reset chapter paragraph formatting (indent, drop caps, etc.) */
        .ebook-body .ebook-page p { text-indent: 0; margin: 0 0 0.6em; hyphens: none; -webkit-hyphens: none; }
        .ebook-body .ebook-page p:empty::before { content: '\\00a0'; }
        .ebook-body .ebook-page p::first-letter { float: none; font-size: inherit; line-height: inherit; margin: 0; padding: 0; }${previewOnly}
    `;

    return inline ? `<style>${css}</style>` : css;
  }

  // Replace every <hr> in `html` with a real <img>-in-div wrapper when the
  // break style is 'image'. Shared by preview and export because pseudo-element
  // sizing on arbitrary image aspect ratios is too unreliable.
  function substituteImageBreaks(html, breaks) {
    if (breaks.style !== 'image' || !breaks.image_data) return html;
    const w = Math.max(1, Math.min(100, Number(breaks.image_width_pct) || 25));
    const mTop = breaks.space_above_em || 0;
    const mBot = breaks.space_below_em || 0;
    const safeSrc = String(breaks.image_data).replace(/"/g, '&quot;');
    const wrapperStyle = `text-align:center;margin:${mTop}em 0 ${mBot}em 0;`;
    const imgStyle = `display:inline-block;width:${w}%;max-width:100%;height:auto;vertical-align:middle;`;
    const replacement = `<div class="scene-break-img" style="${wrapperStyle}"><img src="${safeSrc}" alt="" style="${imgStyle}"/></div>`;
    return html.replace(/<hr\b[^>]*>/g, replacement);
  }

  function generateEbookPreview() {
    if (!chapters || chapters.length === 0) {
      ebookPreviewHtml = '<p style="color:#999;text-align:center;padding:2rem;">No chapters yet</p>';
      return;
    }

    const settings = resolveEbookSettings(activeProfile);
    const css = buildEbookCss(settings, { inline: true });

    let html = css + '<div class="ebook-body">';

    // Front matter pages
    const frontPages = formatPages
      .filter(p => p.position === 'front')
      .sort((a, b) => a.sort_order - b.sort_order);
    for (const page of frontPages) {
      html += `<div id="ebook-fp-${page.id}" class="ebook-chapter-break"></div>`;
      html += `<div class="ebook-page">`;
      if (page.page_role === 'toc') {
        html += generateTocHtml(page);
      } else if (page.page_role === 'map') {
        html += generateMapHtml(page);
      } else {
        html += pageToHtml(page);
      }
      html += '</div>';
    }

    // Chapters
    for (let i = 0; i < chapters.length; i++) {
      const ch = chapters[i];
      html += '<div class="ebook-chapter-break"></div>';
      html += `<div id="ebook-ch-${ch.id}"></div>`;
      html += renderChapterHeadingHtml(ch, i, settings.chHeadings);
      html += chapterToHtml(ch);
    }

    // Back matter pages
    const backPages = formatPages
      .filter(p => p.position === 'back')
      .sort((a, b) => a.sort_order - b.sort_order);
    for (const page of backPages) {
      html += `<div id="ebook-fp-${page.id}" class="ebook-chapter-break"></div>`;
      html += `<div class="ebook-page">`;
      if (page.page_role === 'toc') {
        html += generateTocHtml(page);
      } else if (page.page_role === 'map') {
        html += generateMapHtml(page);
      } else {
        html += pageToHtml(page);
      }
      html += '</div>';
    }

    html += '</div>';
    html = substituteImageBreaks(html, settings.breaks);
    ebookPreviewHtml = html;
  }

  function escapeHtml(s) {
    return (s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  async function compileAndShow() {
    if (!activeProfileId) return;
    if (isEbook) {
      pageCount = 0;
      lastTiming = null;
      teardownObserver();
      loadedPages = new Set();
      generateEbookPreview();
      return;
    }

    // Snapshot the current scroll position so we can restore it after recompile.
    // Fall back to the saved position from project_settings (used on first load
    // and after switching profiles).
    const liveScroll = previewContainer ? previewContainer.scrollTop : null;
    const savedScroll = await loadSavedScroll(activeProfileId);
    const restoreTo = liveScroll && liveScroll > 0 ? liveScroll : savedScroll;

    rendering = true;
    renderError = null;
    try {
      const result = await compilePreview(activeProfileId);
      pageCount = result.page_count;
      lastTiming = result.timing;
      sectionPages = result.section_pages || {};
      compileGeneration++; // bust img cache
      loadedPages = new Set(); // reset — pages will be loaded lazily by observer

      console.log(
        `[format] Compile: ${result.timing.total_ms.toFixed(0)}ms | ` +
        `db:${result.timing.db_load_ms.toFixed(0)} ydoc:${result.timing.ydoc_extract_ms.toFixed(0)} ` +
        `markup:${result.timing.markup_build_ms.toFixed(0)} compile:${result.timing.typst_compile_ms.toFixed(0)} | ` +
        `${result.page_count} pages, ${result.timing.chapter_count} chapters`
      );
    } catch (e) {
      console.error('[format] compile failed:', e);
      renderError = String(e);
      pageCount = 0;
      lastTiming = null;
    } finally {
      rendering = false;
    }

    // Wait two animation frames so:
    // 1) Svelte commits the new pageCount → placeholders mount
    // 2) Browser computes layout (fixed-height placeholders give correct scroll height)
    // Then jump scroll to restore position before the user sees the top.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        restoreScroll(restoreTo);
      });
    });
  }

  async function reloadPages() {
    [formatPages, exclusions] = await Promise.all([getFormatPages(), listPageExclusions()]);
  }

  // Used by Custom mode settings components after they save a category JSON.
  // Refreshes the local profiles array so the prop they receive is up-to-date,
  // then recompiles the preview.
  async function handleCustomSettingChange() {
    profiles = await getFormatProfiles();
    compileAndShow();
  }

  async function loadData() {
    loading = true;
    // Restore sidebar width and clipboard
    const settings = await getSettings();
    if (settings.formatSidebarWidth) sidebarWidth = settings.formatSidebarWidth;
    if (settings.formatClipboard) formatClipboard = settings.formatClipboard;

    await seedFormatProfiles();
    const [profs, chaps] = await Promise.all([getFormatProfiles(), getChapters()]);
    profiles = profs;
    chapters = chaps;

    // Restore last active profile from project settings, or default to first
    if (!activeProfileId) {
      const savedId = await getProjectSetting('format_active_profile');
      const savedNum = Number(savedId);
      if (savedNum && profs.find(p => p.id === savedNum)) {
        activeProfileId = savedNum;
      } else if (profs.length > 0) {
        activeProfileId = profs[0].id;
      }
    }

    await reloadPages();
    loading = false;

    // Kick off rendering after load
    compileAndShow();
  }

  async function switchProfile(id) {
    activeProfileId = id;
    setProjectSetting('format_active_profile', String(id)).catch(() => {});
    compileAndShow();
  }

  // ---- Add page modal ----

  function openAddPageModal(position) {
    addPagePosition = position;
    showAddPageModal = true;
  }

  async function addPageOfType(typeId) {
    const pageType = PAGE_TYPES.find(t => t.id === typeId);
    const title = pageType ? pageType.label : 'New Page';
    await addFormatPage(typeId, title, addPagePosition);
    await reloadPages();
    showAddPageModal = false;
    compileAndShow();
  }

  // ---- Ebook metadata tag handlers ----

  function armTag(tag) {
    if (usedEbookTags.has(tag)) return; // already assigned; X clears it
    armedTag = armedTag === tag ? null : tag;
  }

  async function clearEbookTag(tag) {
    const pageId = usedEbookTags.get(tag);
    if (!pageId) return;
    const page = formatPages.find(p => p.id === pageId);
    if (page) {
      await updateFormatPage(page.id, page.page_role, page.title, page.content, page.position, page.include_in, page.vertical_align, '');
      await reloadPages();
    }
  }

  async function assignEbookTag(item) {
    if (!armedTag) return;
    const tag = armedTag;
    armedTag = null;
    await updateFormatPage(item.id, item.page_role, item.title, item.content, item.position, item.include_in, item.vertical_align, tag);
    await reloadPages();
  }

  function handlePageItemClick(item) {
    if (armedTag) {
      assignEbookTag(item);
    } else {
      scrollToSection(`iwe-fp-${item.id}`);
    }
  }

  function handleChapterItemClick(ch) {
    if (armedTag) {
      armedTag = null;
      return;
    }
    scrollToSection(`iwe-ch-${ch.id}`);
  }

  function handleEscape(e) {
    if (e.key === 'Escape') {
      if (armedTag) armedTag = null;
      if (showAddPageModal) showAddPageModal = false;
      if (editingPageId !== null) editingPageId = null;
    }
  }

  function startEditPage(item) {
    editingPageId = item.id;
    editingPageTitle = item.title || '';
  }

  async function commitEditPage() {
    if (editingPageId === null) return;
    const id = editingPageId;
    const newTitle = editingPageTitle.trim();
    const page = formatPages.find(p => p.id === id);
    editingPageId = null;
    if (!page) return;
    if (newTitle === (page.title || '')) return; // no change
    await updateFormatPage(id, page.page_role, newTitle, page.content, page.position, page.include_in, page.vertical_align, page.ebook_metadata_tag);
    await reloadPages();
    compileAndShow();
  }

  function openPageEditor(item) {
    if (item.page_role === 'toc') {
      editingTocPage = item;
    } else if (item.page_role === 'map') {
      editingMapPage = item;
    } else {
      designingPage = item;
    }
  }

  async function saveDesignedPage({ content: jsonContent, verticalAlign }) {
    if (!designingPage) return;
    const p = designingPage;
    designingPage = null;
    await updateFormatPage(p.id, p.page_role, p.title, jsonContent, p.position, p.include_in, verticalAlign, p.ebook_metadata_tag);
    await reloadPages();
    compileAndShow();
  }

  function cancelDesignedPage() {
    designingPage = null;
  }

  async function saveTocPage({ content: jsonContent }) {
    if (!editingTocPage) return;
    const p = editingTocPage;
    editingTocPage = null;
    await updateFormatPage(p.id, p.page_role, p.title, jsonContent, p.position, p.include_in, p.vertical_align, p.ebook_metadata_tag);
    await reloadPages();
    compileAndShow();
  }

  function cancelTocPage() {
    editingTocPage = null;
  }

  async function saveMapPage({ content: jsonContent }) {
    if (!editingMapPage) return;
    const p = editingMapPage;
    editingMapPage = null;
    await updateFormatPage(p.id, p.page_role, p.title, jsonContent, p.position, p.include_in, p.vertical_align, p.ebook_metadata_tag);
    await reloadPages();
    compileAndShow();
  }

  function cancelMapPage() {
    editingMapPage = null;
  }

  async function handleDeletePage(id) {
    await deleteFormatPage(id);
    await reloadPages();
    compileAndShow();
  }

  // Profile management
  function openCreateProfileModal() {
    newProfileName = '';
    newProfileDuplicateFrom = null;
    newProfileTargetType = 'print';
    showCreateProfileModal = true;
    showProfileMenu = false;
  }

  async function createProfile() {
    const name = newProfileName.trim();
    if (!name) return;

    let newId;
    if (newProfileDuplicateFrom) {
      newId = await duplicateFormatProfile(newProfileDuplicateFrom, name);
    } else {
      // Default sizes per target type
      const w = newProfileTargetType === 'ebook' ? 6.0 : 6.0;
      const h = newProfileTargetType === 'ebook' ? 9.0 : 9.0;
      newId = await addFormatProfile(name, newProfileTargetType, w, h);
    }
    profiles = await getFormatProfiles();
    showCreateProfileModal = false;
    activeProfileId = newId;
    setProjectSetting('format_active_profile', String(newId)).catch(() => {});
    await reloadPages();
    compileAndShow();
  }

  async function copyProfileSettings() {
    if (!activeProfile) return;
    // Snapshot the entire active profile object — Rust filters excluded fields on paste
    formatClipboard = { ...activeProfile };
    // Persist to settings.json so the clipboard survives reloads + project switches
    const settings = await getSettings();
    settings.formatClipboard = formatClipboard;
    await saveSettings(settings);
    addToast(`Copied settings from "${activeProfile.name}"`, 'success');
  }

  async function pasteProfileSettings() {
    if (!activeProfile || !formatClipboard) return;
    if (formatClipboard.id === activeProfile.id) {
      addToast('Cannot paste a profile onto itself', 'info');
      return;
    }
    try {
      await pasteFormatProfileSettings(activeProfile.id, formatClipboard);
      profiles = await getFormatProfiles();
      addToast(`Pasted settings into "${activeProfile.name}"`, 'success');
      compileAndShow();
    } catch (e) {
      console.error('[format] paste failed:', e);
      addToast('Paste failed: ' + e, 'error');
    }
  }

  async function deleteActiveProfile() {
    if (profiles.length <= 1) return;
    const remainingProfile = profiles.find(p => p.id !== activeProfileId);
    await deleteFormatProfile(activeProfileId);
    profiles = await getFormatProfiles();
    activeProfileId = remainingProfile?.id ?? null;
    confirmDeleteProfileId = null;
    showProfileMenu = false;
    compileAndShow();
  }

  function startRenameActive() {
    if (!activeProfile) return;
    renamingProfileId = activeProfile.id;
    renameValue = activeProfile.name;
    showProfileMenu = false;
  }

  async function commitRename() {
    if (!renamingProfileId) return;
    const prof = profiles.find(p => p.id === renamingProfileId);
    if (!prof) { renamingProfileId = null; return; }
    const name = renameValue.trim() || prof.name;
    await updateFormatProfile(
      prof.id, name, prof.target_type,
      prof.trim_width_in, prof.trim_height_in,
      prof.margin_top_in, prof.margin_bottom_in,
      prof.margin_outside_in, prof.margin_inside_in,
      prof.font_body, prof.font_size_pt, prof.line_spacing,
    );
    profiles = await getFormatProfiles();
    renamingProfileId = null;
  }

  // Toggle a page's inclusion in a profile
  async function togglePageInProfile(pageId, profileId) {
    const isIncluded = isPageIncludedIn(pageId, profileId);
    if (isIncluded) {
      await addPageExclusion(pageId, profileId);
    } else {
      await removePageExclusion(pageId, profileId);
    }
    exclusions = await listPageExclusions();
    // Recompile if the change affects the active profile
    if (profileId === activeProfileId) {
      compileAndShow();
    }
  }

  // DnD handlers for front matter
  function handleFrontConsider(e) {
    frontItems = e.detail.items;
  }
  async function handleFrontFinalize(e) {
    frontItems = e.detail.items;
    const ids = frontItems.map(it => it.id);
    await reorderFormatPages(ids);
    await reloadPages();
    compileAndShow();
  }

  // DnD handlers for back matter
  function handleBackConsider(e) {
    backItems = e.detail.items;
  }
  async function handleBackFinalize(e) {
    backItems = e.detail.items;
    const ids = backItems.map(it => it.id);
    await reorderFormatPages(ids);
    await reloadPages();
    compileAndShow();
  }

  // Minimal Image node for HTML generation — mirrors PageContentEditor's ImageNode.
  // PageContentEditor has no alignment attribute on images; centering is done
  // entirely via its NodeView's outer div CSS (text-align: center). In HTML
  // generation there's no NodeView, so we bake the centering into the img's
  // own style. `display: block; margin: * auto` works even under the narrow CSS
  // subsets some EPUB readers allow.
  const ImageNodeForHtml = Node.create({
    name: 'image',
    inline: false,
    group: 'block',
    draggable: true,
    addAttributes() {
      return {
        src: { default: null },
        alt: { default: null },
        width: {
          default: null,
          renderHTML: attrs => attrs.width ? { style: `width: ${attrs.width}` } : {},
        },
      };
    },
    parseHTML() { return [{ tag: 'img[src]' }]; },
    renderHTML({ HTMLAttributes }) {
      return ['img', mergeAttributes(HTMLAttributes, {
        style: 'display: block; margin-left: auto; margin-right: auto;',
      })];
    },
  });

  // Extension set for chapter Y.Doc content. Must include the custom editor nodes
  // (noteMarker, stateMarker, timeBreak) or generateHTML throws on any chapter
  // containing a comment, state marker, or time jump — which silently empties
  // the chapter via chapterToHtml's catch.
  const chapterHtmlExtensions = [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4] }, history: false }),
    TextAlign.configure({ types: ['heading', 'paragraph'] }),
    Superscript,
    Subscript,
    NoteMarker,
    StateMarker,
    TimeBreak,
  ];

  // Extension set for format page content (PM JSON from PageContentEditor).
  // Pages don't use headings or the Y.Doc custom nodes — they use font marks and images.
  const pageHtmlExtensions = [
    StarterKit.configure({ heading: false, history: false }),
    TextStyle,
    FontSize,
    FontFamily,
    ImageNodeForHtml,
    TextAlign.configure({ types: ['paragraph'] }),
  ];

  function chapterToHtml(chapter) {
    try {
      const { doc } = createChapterDoc(chapter.content);
      const json = yDocToProsemirrorJSON(doc, 'prosemirror');
      destroyDoc(doc);
      return generateHTML(json, chapterHtmlExtensions);
    } catch (e) {
      console.error('[format] chapterToHtml failed for chapter', chapter?.id, chapter?.title, e);
      return '<p></p>';
    }
  }

  // Transform HTML (as produced by TipTap's generateHTML) into XHTML-safe
  // form by self-closing HTML void elements. EPUB is strict XHTML and
  // epubcheck RSC-016 fatally rejects `<hr>` and `<img>` without a trailing
  // slash. Browsers tolerate the HTML form, but EPUB parsers won't.
  //
  // Only applied on the EPUB export path. The in-app preview still uses
  // loose HTML because browsers parse it fine and altering it there would
  // just be wasted work.
  function htmlToXhtml(html) {
    if (!html) return '';
    // Per HTML spec, these elements have no content and no end tag.
    const VOID_TAGS = 'area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr';
    // Match `<tag>` or `<tag attr="val">` where the tag is NOT already
    // self-closed. The optional group `(\s[^>]*[^/])?` requires at least one
    // whitespace + at least one non-`/` char before the `>`, so `<tag/>` and
    // `<tag />` are skipped as already-valid.
    const pattern = new RegExp(`<(${VOID_TAGS})(\\s[^>]*[^/])?>`, 'gi');
    return html.replace(pattern, '<$1$2/>');
  }

  // Generate an HTML table of contents from the chapter list.
  // Used for ebook preview and EPUB export when a page has role='toc'.
  function parseTocSettings(page) {
    try {
      const raw = page?.content;
      if (raw && raw.trim().startsWith('{')) {
        const parsed = JSON.parse(raw);
        return {
          toc_title: parsed.toc_title || 'Contents',
          leader_style: parsed.leader_style || 'dots',
          title_font: parsed.title_font || '',
          item_spacing_em: parsed.item_spacing_em ?? 0.5,
          vertical_align: parsed.vertical_align || 'top',
        };
      }
    } catch { /* ignore */ }
    return { toc_title: 'Contents', leader_style: 'dots', title_font: '', item_spacing_em: 0.5, vertical_align: 'top' };
  }

  function generateTocHtml(page) {
    if (!chapters || chapters.length === 0) return '<p>No chapters</p>';
    const settings = parseTocSettings(page);
    const titleFontStyle = settings.title_font ? `font-family: '${settings.title_font}';` : '';
    const spacingPx = Math.round(settings.item_spacing_em * 16);
    const valign = settings.vertical_align === 'center'
      ? 'display: flex; flex-direction: column; justify-content: center; min-height: 80%;'
      : '';
    let html = `<nav class="ebook-toc" style="${valign}">`;
    html += `<h2 style="text-align: center; ${titleFontStyle}">${escapeHtml(settings.toc_title)}</h2>`;
    html += `<ol style="list-style: none; padding: 0;">`;
    for (const ch of chapters) {
      html += `<li style="margin-bottom: ${spacingPx}px;"><a href="#ebook-ch-${ch.id}">${escapeHtml(ch.title)}</a></li>`;
    }
    html += '</ol></nav>';
    return html;
  }

  function generateMapHtml(page) {
    try {
      const parsed = JSON.parse(page.content);
      if (!parsed.image_data) return '<p>No map image</p>';
      return `<div class="ebook-map" style="text-align: center;"><img src="${parsed.image_data}" alt="Map" style="max-width: 100%; height: auto;" /></div>`;
    } catch { return '<p>No map image</p>'; }
  }

  // Convert a format_pages row's stored content to HTML. Content is one of:
  //   - empty string
  //   - ProseMirror JSON (stringified) from PageContentEditor — the normal case
  //   - legacy plain text (paragraphs separated by \n\n)
  function pageToHtml(page) {
    const raw = page?.content;
    if (!raw || !raw.trim()) return '';
    if (raw.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(raw);
        return generateHTML(parsed, pageHtmlExtensions);
      } catch (e) {
        console.error('[format] pageToHtml failed for page', page?.id, page?.title, e);
        return '';
      }
    }
    // Legacy plain text fallback
    return raw.split('\n\n')
      .map(p => `<p style="text-indent:0;">${escapeHtml(p.trim())}</p>`)
      .join('\n');
  }

  // ---- Export size estimation ----
  //
  // Computes an estimate of the resulting EPUB size WITHOUT actually running
  // the export. Dominated by images (cover + inline) which we measure exactly
  // from their byte representation; XHTML text uses a DEFLATE estimate.
  //
  // The estimate is accurate within ~5% for typical books. It does NOT account
  // for any future image compression — for that, we'd need to actually run the
  // encoder. This is intentionally a lower bound on "what you'd get today".

  // Every EPUB ships with these fixed files. The sizes are approximate (they
  // vary with chapter count and metadata) but constant enough that we can
  // treat them as a fixed ~10KB baseline.
  const FIXED_EPUB_OVERHEAD_BYTES = 10 * 1024;

  // DEFLATE typically compresses well-structured XHTML to ~30% of source.
  // This is a rough rule of thumb; actual ratios range from 20% (lots of
  // repeated markup) to 40% (dense text with little markup).
  const XHTML_COMPRESSION_RATIO = 0.3;

  // Decode the byte length of a base64 string without actually decoding.
  // Each 4 base64 chars encode 3 bytes; padding `=` at the end reduces
  // the output by 1 or 2 bytes.
  function base64DecodedLength(b64) {
    if (!b64) return 0;
    const len = b64.length;
    if (len === 0) return 0;
    let padding = 0;
    if (b64[len - 1] === '=') padding++;
    if (b64[len - 2] === '=') padding++;
    return Math.floor(len * 3 / 4) - padding;
  }

  // Scan an HTML string for inline base64 data URL images. Collects unique
  // images into the `seen` Map (key → { dataUrl, mime, bytes }) so the
  // caller can later read their dimensions for compression estimation.
  // Also returns the total base64 character length in THIS HTML — not
  // deduped — so the text estimator can subtract it from html.length to
  // reflect the actual markup that ships after image extraction.
  function measureInlineImages(html, seen) {
    if (!html) return { base64CharsInHtml: 0 };
    const re = /<img\b[^>]*\ssrc="(data:([^;]+);base64,([^"]+))"/gi;
    let base64CharsInHtml = 0;
    let match;
    while ((match = re.exec(html)) !== null) {
      const dataUrl = match[1];
      const mime = match[2];
      const b64 = match[3];
      // Every occurrence counted for the text subtraction — duplicates all
      // contribute their full base64 to the source HTML length, and all of
      // them get replaced with short paths during export.
      base64CharsInHtml += b64.length;
      // Dedupe via a short key so the image-bytes accumulator matches what
      // the Rust side actually stores in the zip (each unique image once).
      const key = mime + ';' + b64.substring(0, 64) + ':' + b64.length;
      if (seen.has(key)) continue;
      seen.set(key, { dataUrl, mime, bytes: base64DecodedLength(b64), width: 0, height: 0 });
    }
    return { base64CharsInHtml };
  }

  // Read the natural dimensions of a data URL by letting the browser decode
  // just the image header. Fast (milliseconds) and non-blocking.
  function loadImageDimensions(dataUrl) {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
      img.onerror = () => resolve(null);
      img.src = dataUrl;
    });
  }

  // Compression presets. `max_dim` caps the longest edge; `target_bpp` is
  // an assumed bytes-per-pixel floor for JPEG encoding at the preset's
  // quality level. These bpp values are middle-ground for photographic
  // content — simple graphics compress much smaller (~0.1 bpp), detailed
  // photos somewhat larger.
  const COMPRESSION_PRESETS = {
    none:     { max_dim: Infinity, target_bpp: Infinity, factor_fallback: 1.0 },
    minor:    { max_dim: 2500,     target_bpp: 0.5,      factor_fallback: 0.75 },
    balanced: { max_dim: 2000,     target_bpp: 0.35,     factor_fallback: 0.5 },
    compact:  { max_dim: 1500,     target_bpp: 0.18,     factor_fallback: 0.25 },
  };

  // Estimate the post-compression bytes for a single image. Uses actual
  // dimensions when available — the downscale contribution (new_pixels /
  // old_pixels) is the dominant factor for large source images, and we
  // compute it exactly. The quality contribution caps the result at
  // `new_pixels * target_bpp` (what a fresh Q85/Q70 encode would produce
  // regardless of source), but also caps at original bytes-per-pixel
  // since re-encoding can't magically increase information density.
  //
  // When dimensions aren't available (image header failed to decode) or
  // level is "none", falls back to a flat multiplier.
  function estimateImageBytes(img, level) {
    const preset = COMPRESSION_PRESETS[level] || COMPRESSION_PRESETS.none;
    if (level === 'none' || !img.width || !img.height) {
      return Math.ceil(img.bytes * preset.factor_fallback);
    }
    const longest = Math.max(img.width, img.height);
    const scale = longest > preset.max_dim ? preset.max_dim / longest : 1;
    const new_pixels = img.width * img.height * scale * scale;
    // original bytes-per-pixel — caps the target_bpp so we never "expand"
    // a source that was already encoded smaller than the target quality
    const original_bpp = img.bytes / (img.width * img.height);
    const effective_bpp = Math.min(original_bpp, preset.target_bpp);
    return Math.ceil(new_pixels * effective_bpp);
  }

  // Estimate the total size of an EPUB export for the currently active
  // ebook profile. Returns a breakdown + compression metadata.
  //
  // How it works for compression estimates:
  //   1. Scan all chapter/page HTML for inline data: URLs, collect unique
  //      ones into a Map keyed by a short mime+b64 fingerprint
  //   2. Read each unique image's natural dimensions via `new Image()`
  //      (fast — browser decodes only the header)
  //   3. For each image, compute estimated post-compression bytes using
  //      actual downscale ratio × quality bpp floor. This is far more
  //      accurate than a flat multiplier because the downscale contribution
  //      is the dominant factor for large source images.
  //   4. Same treatment for the cover (read its dimensions via an Image()).
  //
  // The text portion is html.length - base64_chars_in_html (since the base64
  // payload gets extracted and replaced with short paths on the Rust side),
  // then multiplied by the DEFLATE ratio.
  async function estimateExportSize(level = 'none') {
    if (!activeProfile || activeProfile.target_type !== 'ebook') {
      return null;
    }

    // Map<key, { dataUrl, mime, bytes, width, height }> of unique inline images
    const seen = new Map();
    let markupBytes = 0;

    // Chapter bodies — collect images + count markup
    for (const ch of chapters || []) {
      try {
        const html = chapterToHtml(ch);
        const { base64CharsInHtml } = measureInlineImages(html, seen);
        markupBytes += Math.max(0, html.length - base64CharsInHtml);
      } catch {
        // Swallow — already logged by chapterToHtml
      }
    }

    // Format page bodies
    for (const p of formatPages || []) {
      try {
        const html = pageToHtml(p);
        const { base64CharsInHtml } = measureInlineImages(html, seen);
        markupBytes += Math.max(0, html.length - base64CharsInHtml);
      } catch {
        // Swallow
      }
    }

    // Cover image (BLOB in project DB). Read bytes + dimensions.
    let coverImage = null;
    try {
      const cover = await getBookCover();
      if (cover && cover.data && cover.data.length > 0) {
        // Build a data URL from the bytes so loadImageDimensions can decode it.
        // Uint8Array → base64 via a chunked approach to avoid stack overflow
        // on large images.
        const bytes = new Uint8Array(cover.data);
        let binary = '';
        const chunk = 0x8000;
        for (let i = 0; i < bytes.length; i += chunk) {
          binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
        }
        const dataUrl = `data:${cover.mime_type};base64,${btoa(binary)}`;
        coverImage = {
          dataUrl,
          mime: cover.mime_type,
          bytes: cover.data.length,
          width: 0,
          height: 0,
        };
      }
    } catch (e) {
      console.warn('[format] estimateExportSize: cover read failed', e);
    }

    // Read dimensions for all unique inline images + the cover in parallel.
    // `new Image()` decodes the header only — fast and non-blocking.
    const allImages = Array.from(seen.values());
    if (coverImage) allImages.push(coverImage);
    await Promise.all(
      allImages.map(async (img) => {
        const dims = await loadImageDimensions(img.dataUrl);
        if (dims) {
          img.width = dims.width;
          img.height = dims.height;
        }
      })
    );

    // Tally raw bytes + estimated compressed bytes
    let rawImagesBytes = 0;
    let estImagesBytes = 0;
    for (const img of seen.values()) {
      rawImagesBytes += img.bytes;
      estImagesBytes += estimateImageBytes(img, level);
    }
    const rawCoverBytes = coverImage ? coverImage.bytes : 0;
    const estCoverBytes = coverImage ? estimateImageBytes(coverImage, level) : 0;

    // XHTML compresses via DEFLATE to roughly 30% of source.
    const compressedText = Math.ceil(markupBytes * XHTML_COMPRESSION_RATIO);

    // Total
    const total = estCoverBytes + estImagesBytes + compressedText + FIXED_EPUB_OVERHEAD_BYTES;

    return {
      total,
      breakdown: {
        cover: estCoverBytes,
        images: estImagesBytes,
        text: compressedText,
        overhead: FIXED_EPUB_OVERHEAD_BYTES,
      },
      imageCount: seen.size,
      rawImagesBytes,
      rawCoverBytes,
      compressionLevel: level,
    };
  }

  // Format bytes as a human-readable string ("4.2 MB", "540 KB", "8.1 GB").
  function formatBytes(n) {
    if (n == null || n === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    let i = 0;
    let v = n;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i++;
    }
    return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
  }

  async function handleExportEpub() {
    if (exporting) return;
    exporting = true;
    exportError = null;
    try {
      const bookTitle = await getProjectSetting('book_title') || 'Untitled';
      const authorName = await getProjectSetting('author_name') || 'Unknown Author';
      const bookLanguage = (await getProjectSetting('language') || 'en').trim() || 'en';
      const bookDescription = (await getProjectSetting('description') || '').trim();

      // Resolve profile settings once so chapters and pages share the same
      // chapter-heading rendering, image-break substitution, and CSS.
      const settings = resolveEbookSettings(activeProfile);
      const css = buildEbookCss(settings, { inline: false });

      // Build each chapter's full HTML: the JS-rendered chapter heading
      // (number/title/subtitle/image/rules honoring chapter_headings_json)
      // + the chapter body. Rust's build_chapter_html no longer adds its own
      // h1 — we provide everything here.
      const epubChapters = chapters.map((ch, i) => {
        const heading = renderChapterHeadingHtml(ch, i, settings.chHeadings);
        const body = chapterToHtml(ch);
        const combined = substituteImageBreaks(heading + body, settings.breaks);
        return {
          title: ch.title,
          subtitle: ch.subtitle || '',
          html: htmlToXhtml(combined),
        };
      });

      const allPages = formatPages;
      const epubPageContent = (p) => {
        if (p.page_role === 'toc') return generateTocHtml(p);
        if (p.page_role === 'map') return generateMapHtml(p);
        return pageToHtml(p);
      };
      const frontPages = allPages
        .filter(p => p.position === 'front')
        .map(p => ({
          title: p.title,
          role: p.ebook_metadata_tag || p.page_role,
          html: htmlToXhtml(substituteImageBreaks(`<div class="ebook-page">${epubPageContent(p)}</div>`, settings.breaks)),
          position: p.position,
        }));
      const backPages = allPages
        .filter(p => p.position === 'back')
        .map(p => ({
          title: p.title,
          role: p.ebook_metadata_tag || p.page_role,
          html: htmlToXhtml(substituteImageBreaks(`<div class="ebook-page">${epubPageContent(p)}</div>`, settings.breaks)),
          position: p.position,
        }));

      // cover_image is NOT sent from JS — Rust reads the cover BLOB directly
      // from the book_cover table so we don't round-trip binary through JSON.
      const request = {
        title: bookTitle,
        author: authorName,
        language: bookLanguage,
        description: bookDescription,
        chapters: epubChapters,
        front_pages: frontPages,
        back_pages: backPages,
        css,
        compression_level: compressionLevel,
      };

      const epubBytes = await exportEpub(request);
      const arr = epubBytes instanceof Uint8Array ? epubBytes : new Uint8Array(epubBytes);

      // Run the Rust-side sanity checker. This isn't a full epubcheck
      // replacement — it catches the class of bugs we've hit in this
      // codebase (bad XHTML, duplicate OPF ids, dangling manifest refs).
      // Errors block the save with a toast explaining what's wrong;
      // warnings surface but don't block.
      const issues = await validateEpubBytes(arr);
      const errors = issues.filter(i => i.level === 'error');
      const warnings = issues.filter(i => i.level === 'warning');

      if (errors.length > 0) {
        console.error('[format] EPUB validation errors:', errors);
        const summary = errors.slice(0, 3).map(e => `${e.code}: ${e.message}`).join(' | ');
        const more = errors.length > 3 ? ` (+${errors.length - 3} more)` : '';
        exportError = `Validation failed (${errors.length} error${errors.length > 1 ? 's' : ''}): ${summary}${more}`;
        addToast(`EPUB validation failed — ${errors.length} error${errors.length > 1 ? 's' : ''}. See console for details.`, 'error');
        // Still fall through to save so the user has the bytes to inspect.
      } else if (warnings.length > 0) {
        console.warn('[format] EPUB validation warnings:', warnings);
        addToast(`EPUB exported with ${warnings.length} warning${warnings.length > 1 ? 's' : ''}`, 'info');
      }

      const filePath = await save({
        title: 'Save EPUB',
        defaultPath: `${bookTitle}.epub`,
        filters: [{ name: 'EPUB', extensions: ['epub'] }],
      });

      if (filePath) {
        const { writeFile } = await import('@tauri-apps/plugin-fs');
        await writeFile(filePath, arr);
        if (errors.length === 0) {
          addToast(`EPUB exported to ${filePath.split(/[/\\]/).pop()}`, 'success');
        } else {
          addToast(`EPUB saved (with errors) to ${filePath.split(/[/\\]/).pop()}`, 'error');
        }
      }
    } catch (e) {
      console.error('[format] epub export failed:', e);
      exportError = String(e);
      addToast('EPUB export failed: ' + e, 'error');
    } finally {
      exporting = false;
    }
  }

  async function handleExportPdf() {
    if (exporting || pageCount === 0) return;
    exporting = true;
    exportError = null;
    try {
      // Ensure we have a fresh compile
      if (!pageCount) await compileAndShow();

      const pdfBytes = await exportFormatPdf();
      const arr = pdfBytes instanceof Uint8Array ? pdfBytes : new Uint8Array(pdfBytes);

      // Save dialog
      const profileName = activeProfile?.name || 'export';
      const filePath = await save({
        title: 'Save PDF',
        defaultPath: `${profileName}.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });

      if (filePath) {
        const { writeFile } = await import('@tauri-apps/plugin-fs');
        await writeFile(filePath, arr);
        addToast(`PDF exported to ${filePath.split(/[/\\]/).pop()}`, 'success');
      }
    } catch (e) {
      console.error('[format] export failed:', e);
      exportError = String(e);
      addToast('Export failed: ' + e, 'error');
    } finally {
      exporting = false;
    }
  }

  function scrollToPage(index) {
    const el = document.getElementById(`preview-page-${index}`);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function scrollToSection(sectionId) {
    if (isEbook) {
      // For ebook, scroll within the ebook preview to the anchor
      const chMatch = sectionId.match(/iwe-ch-(\d+)/);
      const fpMatch = sectionId.match(/iwe-fp-(\d+)/);
      let el = null;
      if (chMatch) el = document.getElementById(`ebook-ch-${chMatch[1]}`);
      else if (fpMatch) el = document.getElementById(`ebook-fp-${fpMatch[1]}`);
      if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      return;
    }
    const idx = sectionPages[sectionId];
    if (idx == null) return;
    scrollToPage(idx);
  }

  let hasLoaded = false;
  onMount(loadData);

  // Recompile whenever the user navigates TO this page (e.g. after editing
  // chapter images/content in the editor tab). afterNavigate fires on every
  // route transition including the initial load — we skip the first one since
  // onMount already handles it.
  afterNavigate(() => {
    if (hasLoaded) {
      compileAndShow();
    }
    hasLoaded = true;
  });

  onDestroy(() => {
    teardownObserver();
    clearTimeout(scrollIdleTimer);
  });
</script>

<svelte:window onkeydown={handleEscape} />

{#if loading}
  <div class="format-loading">
    <div class="loader"></div>
    <p>Loading formatting...</p>
  </div>
{:else}
  <div class="format-layout">
    <!-- Center: Preview + timing -->
    <div class="preview-column">
    <div class="preview-area" bind:this={previewContainer} onscroll={handlePreviewScroll}>
      {#if isEbook}
        <div class="ebook-preview-wrap">
          <div class="ebook-device" style="width: {deviceDims.w + 24}px;">
            <div class="ebook-screen" style="width: {deviceDims.w}px; height: {deviceDims.h}px;">
              {@html ebookPreviewHtml}
            </div>
          </div>
        </div>
      {:else if rendering}
        <div class="render-loading">
          <div class="loader"></div>
          <p>Compiling pages...</p>
        </div>
      {:else if renderError}
        <div class="render-error">
          <i class="bi bi-exclamation-triangle"></i>
          <p>Rendering failed</p>
          <pre class="error-detail">{renderError}</pre>
          <button class="retry-btn" onclick={compileAndShow}>Retry</button>
        </div>
      {:else if pageCount === 0}
        <div class="render-loading">
          <p>No pages to display</p>
        </div>
      {:else}
        <div class="preview-scroll">
          {#each Array(pageCount) as _, i}
            <div class="preview-page-wrap" id="preview-page-{i}" data-page-index={i}>
              {#if loadedPages.has(i)}
                <img
                  class="preview-page-img"
                  src="http://iwe.localhost/preview/page/{i}.svg?v={compileGeneration}"
                  alt="Page {i + 1}"
                  draggable="false"
                />
              {:else}
                <div class="preview-page-placeholder"
                  style="width: {(activeProfile?.trim_width_in ?? 6) * 72}px; aspect-ratio: {activeProfile?.trim_width_in ?? 6} / {activeProfile?.trim_height_in ?? 9};"></div>
              {/if}
              <div class="page-number">{i + 1}</div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    {#if lastTiming}
      <div class="timing-bar">
        <span title="Total compile time">{lastTiming.total_ms.toFixed(0)}ms</span>
        <span class="timing-sep">|</span>
        <span title="DB load">db:{lastTiming.db_load_ms.toFixed(0)}</span>
        <span title="Y.Doc text extraction">ydoc:{lastTiming.ydoc_extract_ms.toFixed(0)}</span>
        <span title="Typst markup generation">markup:{lastTiming.markup_build_ms.toFixed(0)}</span>
        <span title="Typst compilation">compile:{lastTiming.typst_compile_ms.toFixed(0)}</span>
        <span class="timing-sep">|</span>
        <span>{lastTiming.page_count} pages</span>
      </div>
    {/if}
    </div><!-- end preview-column -->

    <!-- Resize handle -->
    <div
      class="resize-handle"
      class:active={dragging}
      role="separator"
      aria-orientation="vertical"
      onmousedown={startDrag}
    ></div>

    <!-- Right sidebar -->
    <div class="format-sidebar" style="width: {sidebarWidth}px;">
      <!-- Profile selector -->
      <div class="sidebar-section profile-section">
        <div class="profile-row">
          {#if renamingProfileId === activeProfileId}
            <input class="profile-rename-input"
              bind:value={renameValue}
              onblur={commitRename}
              onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renamingProfileId = null; }}
              autofocus />
          {:else}
            <select class="profile-select"
              value={activeProfileId}
              onchange={(e) => switchProfile(Number(e.target.value))}>
              {#each profiles as p}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          {/if}
          <button class="profile-action-btn" title="Rename" onclick={startRenameActive}>
            <i class="bi bi-pencil"></i>
          </button>
          <button class="profile-action-btn" title="Copy settings (excludes target/size)"
            onclick={copyProfileSettings}>
            <i class="bi bi-clipboard"></i>
          </button>
          <button class="profile-action-btn"
            title={formatClipboard
              ? `Paste settings from "${formatClipboard.name}" (target/size unchanged)`
              : 'Nothing to paste — copy a profile first'}
            disabled={!formatClipboard || formatClipboard.id === activeProfileId}
            onclick={pasteProfileSettings}>
            <i class="bi bi-clipboard-check"></i>
          </button>
          <button class="profile-action-btn" title="New profile" onclick={openCreateProfileModal}>
            <i class="bi bi-plus-lg"></i>
          </button>
          <button class="profile-action-btn danger"
            title={profiles.length <= 1 ? 'Cannot delete the only profile' : 'Delete profile'}
            disabled={profiles.length <= 1}
            onclick={() => confirmDeleteProfileId = activeProfileId}>
            <i class="bi bi-trash"></i>
          </button>
        </div>
        {#if confirmDeleteProfileId === activeProfileId}
          <div class="confirm-delete">
            Delete "{activeProfile?.name}"?
            <button class="confirm-yes" onclick={deleteActiveProfile}>Yes</button>
            <button class="confirm-no" onclick={() => confirmDeleteProfileId = null}>No</button>
          </div>
        {/if}
      </div>

      <!-- Mode selector -->
      <div class="sidebar-section mode-section">
        <div class="mode-tabs">
          {#each SIDEBAR_MODES as mode}
            <button class="mode-tab" class:active={sidebarMode === mode.key}
              onclick={() => sidebarMode = mode.key}>
              <i class="bi {mode.icon}"></i>
              <span>{mode.label}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Mode content -->
      <div class="sidebar-mode-content">
        {#if sidebarMode === 'pages'}
          <div class="mode-panel page-list-panel">
            <!-- Ebook metadata tags — only for ebook profiles -->
            {#if isEbook}
              <div class="tag-bar">
                {#each EBOOK_TAGS as tag}
                  {@const used = usedEbookTags.has(tag)}
                  <button class="tag-pill"
                    class:used
                    class:armed={armedTag === tag}
                    title={used ? `Assigned — click × to clear` : (armedTag === tag ? 'Click a page to assign this tag (Esc to cancel)' : `Tag a page as ${ebookTagLabel(tag)}`)}
                    onclick={() => armTag(tag)}>
                    <span>{ebookTagLabel(tag)}</span>
                    {#if used}
                      <span class="tag-x" title="Clear this tag"
                        onclick={(e) => { e.stopPropagation(); clearEbookTag(tag); }}>×</span>
                    {/if}
                  </button>
                {/each}
              </div>
              {#if armedTag}
                <div class="armed-hint">Click a page to tag it as <strong>{ebookTagLabel(armedTag)}</strong> · <kbd>Esc</kbd> to cancel</div>
              {/if}
            {/if}

            <!-- Front matter (draggable) -->
            <div class="page-group-label">
              Front Matter
              <button class="add-page-btn" title="Add front matter page"
                onclick={() => openAddPageModal('front')}>
                <i class="bi bi-plus"></i>
              </button>
            </div>
            <div class="dnd-zone"
              use:dndzone={{ items: frontItems, flipDurationMs, type: 'front-pages', dropTargetStyle: { outline: '2px dashed var(--iwe-accent)', 'outline-offset': '-2px' } }}
              onconsider={handleFrontConsider}
              onfinalize={handleFrontFinalize}>
              {#each frontItems as item (item.id)}
                <div class="page-list-entry" animate:flip={{ duration: flipDurationMs }}>
                  <div class="page-list-item format-page-item" class:armed-target={armedTag && isEbook} onclick={() => editingPageId === item.id ? null : handlePageItemClick(item)}>
                    <i class="bi bi-grip-vertical drag-handle"></i>
                    {#if editingPageId === item.id}
                      <input class="page-item-input"
                        bind:value={editingPageTitle}
                        onblur={commitEditPage}
                        onkeydown={(e) => { if (e.key === 'Enter') commitEditPage(); }}
                        onclick={(e) => e.stopPropagation()}
                        autofocus />
                    {:else}
                      <span class="page-item-title">{item.title || pageTypeLabel(item.page_role)}</span>
                    {/if}
                    <span class="page-item-role">{pageTypeLabel(item.page_role)}</span>
                    {#if isEbook && item.ebook_metadata_tag}
                      <span class="page-item-ebook-tag" title="Ebook tag: {ebookTagLabel(item.ebook_metadata_tag)}">{ebookTagLabel(item.ebook_metadata_tag)}</span>
                    {/if}
                    <button class="page-item-edit" title="Edit content"
                      onclick={(e) => { e.stopPropagation(); openPageEditor(item); }}>
                      <i class="bi {item.page_role === 'toc' ? 'bi-gear' : item.page_role === 'map' ? 'bi-map' : 'bi-card-text'}"></i>
                    </button>
                    <button class="page-item-edit" title="Rename"
                      onclick={(e) => { e.stopPropagation(); startEditPage(item); }}>
                      <i class="bi bi-pencil"></i>
                    </button>
                    <button class="page-item-delete" title="Remove page"
                      onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                      <i class="bi bi-x"></i>
                    </button>
                  </div>
                  <div class="profile-pills">
                    {#each profiles as p}
                      <button class="profile-pill"
                        class:included={isPageIncludedIn(item.id, p.id)}
                        title={isPageIncludedIn(item.id, p.id) ? `Included in ${p.name}` : `Excluded from ${p.name}`}
                        onclick={(e) => { e.stopPropagation(); togglePageInProfile(item.id, p.id); }}>
                        {p.name}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>

            <!-- Chapters (locked, non-draggable) -->
            <div class="page-group-label">Chapters</div>
            {#each chapters as ch (ch.id)}
              <div class="page-list-item chapter-item"
                onclick={() => handleChapterItemClick(ch)}>
                <span class="page-item-title">{ch.title}</span>
              </div>
            {/each}

            <!-- Back matter (draggable) -->
            <div class="page-group-label">
              Back Matter
              <button class="add-page-btn" title="Add back matter page"
                onclick={() => openAddPageModal('back')}>
                <i class="bi bi-plus"></i>
              </button>
            </div>
            <div class="dnd-zone"
              use:dndzone={{ items: backItems, flipDurationMs, type: 'back-pages', dropTargetStyle: { outline: '2px dashed var(--iwe-accent)', 'outline-offset': '-2px' } }}
              onconsider={handleBackConsider}
              onfinalize={handleBackFinalize}>
              {#each backItems as item (item.id)}
                <div class="page-list-entry" animate:flip={{ duration: flipDurationMs }}>
                  <div class="page-list-item format-page-item" class:armed-target={armedTag && isEbook} onclick={() => editingPageId === item.id ? null : handlePageItemClick(item)}>
                    <i class="bi bi-grip-vertical drag-handle"></i>
                    {#if editingPageId === item.id}
                      <input class="page-item-input"
                        bind:value={editingPageTitle}
                        onblur={commitEditPage}
                        onkeydown={(e) => { if (e.key === 'Enter') commitEditPage(); }}
                        onclick={(e) => e.stopPropagation()}
                        autofocus />
                    {:else}
                      <span class="page-item-title">{item.title || pageTypeLabel(item.page_role)}</span>
                    {/if}
                    <span class="page-item-role">{pageTypeLabel(item.page_role)}</span>
                    {#if isEbook && item.ebook_metadata_tag}
                      <span class="page-item-ebook-tag" title="Ebook tag: {ebookTagLabel(item.ebook_metadata_tag)}">{ebookTagLabel(item.ebook_metadata_tag)}</span>
                    {/if}
                    <button class="page-item-edit" title="Edit content"
                      onclick={(e) => { e.stopPropagation(); openPageEditor(item); }}>
                      <i class="bi {item.page_role === 'toc' ? 'bi-gear' : item.page_role === 'map' ? 'bi-map' : 'bi-card-text'}"></i>
                    </button>
                    <button class="page-item-edit" title="Rename"
                      onclick={(e) => { e.stopPropagation(); startEditPage(item); }}>
                      <i class="bi bi-pencil"></i>
                    </button>
                    <button class="page-item-delete" title="Remove page"
                      onclick={(e) => { e.stopPropagation(); handleDeletePage(item.id); }}>
                      <i class="bi bi-x"></i>
                    </button>
                  </div>
                  <div class="profile-pills">
                    {#each profiles as p}
                      <button class="profile-pill"
                        class:included={isPageIncludedIn(item.id, p.id)}
                        title={isPageIncludedIn(item.id, p.id) ? `Included in ${p.name}` : `Excluded from ${p.name}`}
                        onclick={(e) => { e.stopPropagation(); togglePageInProfile(item.id, p.id); }}>
                        {p.name}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {:else if sidebarMode === 'themes'}
          <div class="mode-panel">
            <p class="shell-placeholder">Theme presets will appear here.</p>
          </div>
        {:else if sidebarMode === 'custom'}
          <div class="mode-panel">
            <!-- Sub-tab selector dropdown -->
            <div class="custom-selector-wrap">
              <button class="custom-selector-btn"
                onclick={() => customSelectorOpen = !customSelectorOpen}>
                <i class="bi {activeCustomTab.icon}"></i>
                <span class="custom-selector-label">{activeCustomTab.label}</span>
                <i class="bi bi-chevron-down custom-selector-chevron" class:open={customSelectorOpen}></i>
              </button>
              {#if customSelectorOpen}
                <div class="custom-selector-backdrop"
                  onclick={() => customSelectorOpen = false}
                  role="button" tabindex="-1" onkeydown={() => {}}></div>
                <div class="custom-selector-dropdown">
                  {#each filteredCustomTabs as tab (tab.id)}
                    <button class="custom-option"
                      class:active={customTab === tab.id}
                      onclick={() => selectCustomTab(tab.id)}>
                      <i class="bi {tab.icon}"></i>
                      <span>{tab.label}</span>
                      {#if customTab === tab.id}
                        <i class="bi bi-check2 custom-option-check"></i>
                      {/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>

            <!-- Active sub-tab component -->
            {#if customTab === 'chapter-headings'}
              <ChapterHeadings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'paragraph'}
              <ParagraphSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'headings'}
              <HeadingsSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'breaks'}
              <BreaksSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'print-layout'}
              <PrintLayoutSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'typography'}
              <TypographySettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'header-footer'}
              <HeaderFooterSettings profile={activeProfile} onchange={handleCustomSettingChange} />
            {:else if customTab === 'trim'}
              <TrimSettings profile={activeProfile} onchange={handleCustomSettingChange} bind:ebookDevice />
            {/if}
          </div>
        {:else if sidebarMode === 'export'}
          <div class="mode-panel">
            <div class="export-panel">
              <h4 class="export-title">Export</h4>

              {#if activeProfile}
                <div class="export-info">
                  <div class="export-info-row">
                    <span class="export-info-label">Profile</span>
                    <span class="export-info-value">{activeProfile.name}</span>
                  </div>
                  {#if !isEbook}
                    <div class="export-info-row">
                      <span class="export-info-label">Trim</span>
                      <span class="export-info-value">{activeProfile.trim_width_in}″ × {activeProfile.trim_height_in}″</span>
                    </div>
                    <div class="export-info-row">
                      <span class="export-info-label">Pages</span>
                      <span class="export-info-value">{pageCount || '—'}</span>
                    </div>
                  {:else}
                    <div class="export-info-row">
                      <span class="export-info-label">Estimated size</span>
                      <span class="export-info-value">
                        {#if estimating}
                          <span class="export-info-muted">calculating…</span>
                        {:else if sizeEstimate}
                          ~{formatBytes(sizeEstimate.total)}
                          <span class="estimate-badge" title="Rough estimate. Actual size depends on DEFLATE compression of the XHTML and per-image zip overhead — expect ±5% for uncompressed exports, ±30% when image compression is on.">rough</span>
                        {:else}
                          —
                        {/if}
                      </span>
                    </div>
                  {/if}
                </div>

                {#if isEbook}
                  <div class="export-compression">
                    <div class="compression-title">Image compression</div>
                    <div class="compression-options">
                      <label class="compression-option" class:active={compressionLevel === 'none'}>
                        <input type="radio" name="compression" value="none"
                          checked={compressionLevel === 'none'}
                          onchange={() => compressionLevel = 'none'} />
                        <div class="compression-option-body">
                          <span class="compression-option-name">None</span>
                          <span class="compression-option-desc">Original image bytes, no re-encoding.</span>
                        </div>
                      </label>
                      <label class="compression-option" class:active={compressionLevel === 'minor'}>
                        <input type="radio" name="compression" value="minor"
                          checked={compressionLevel === 'minor'}
                          onchange={() => compressionLevel = 'minor'} />
                        <div class="compression-option-body">
                          <span class="compression-option-name">Minor</span>
                          <span class="compression-option-desc">JPEG Q90, max 2500px. Light touch — preserves quality, trims oversized images.</span>
                        </div>
                      </label>
                      <label class="compression-option" class:active={compressionLevel === 'balanced'}>
                        <input type="radio" name="compression" value="balanced"
                          checked={compressionLevel === 'balanced'}
                          onchange={() => compressionLevel = 'balanced'} />
                        <div class="compression-option-body">
                          <span class="compression-option-name">Balanced</span>
                          <span class="compression-option-desc">JPEG Q85, max 2000px. Usually shrinks images ~50%.</span>
                        </div>
                      </label>
                      <label class="compression-option" class:active={compressionLevel === 'compact'}>
                        <input type="radio" name="compression" value="compact"
                          checked={compressionLevel === 'compact'}
                          onchange={() => compressionLevel = 'compact'} />
                        <div class="compression-option-body">
                          <span class="compression-option-name">Compact</span>
                          <span class="compression-option-desc">JPEG Q70, max 1500px. Aggressive, ~75% reduction.</span>
                        </div>
                      </label>
                    </div>
                    <p class="compression-hint">
                      Compression re-encodes every image when you hit Export. PNG images with transparency are kept as PNG to preserve alpha; everything else becomes JPEG.
                    </p>
                  </div>
                {/if}

                {#if isEbook && sizeEstimate}
                  <div class="export-size-breakdown">
                    <div class="breakdown-title">
                      Where the bytes go
                      <span class="breakdown-title-badge">estimate</span>
                    </div>
                    {#each [
                      { label: 'Cover image',     bytes: sizeEstimate.breakdown.cover,    color: '#2d6a5e' },
                      { label: `Inline images${sizeEstimate.imageCount ? ` (${sizeEstimate.imageCount})` : ''}`, bytes: sizeEstimate.breakdown.images,   color: '#d97706' },
                      { label: 'Text (compressed)', bytes: sizeEstimate.breakdown.text,  color: '#6b6560' },
                      { label: 'EPUB overhead',   bytes: sizeEstimate.breakdown.overhead, color: '#c0b8a8' },
                    ] as row}
                      {#if row.bytes > 0}
                        <div class="breakdown-row">
                          <span class="breakdown-dot" style="background:{row.color}"></span>
                          <span class="breakdown-label">{row.label}</span>
                          <span class="breakdown-value">{formatBytes(row.bytes)}</span>
                        </div>
                      {/if}
                    {/each}
                    <div class="breakdown-row breakdown-total">
                      <span class="breakdown-dot breakdown-dot-total"></span>
                      <span class="breakdown-label">Total</span>
                      <span class="breakdown-value">~{formatBytes(sizeEstimate.total)}</span>
                    </div>
                    {#if compressionLevel !== 'none' && sizeEstimate.rawImagesBytes > 0}
                      <div class="breakdown-savings">
                        <i class="bi bi-arrow-down-right"></i>
                        Down from ~{formatBytes(sizeEstimate.rawImagesBytes + sizeEstimate.rawCoverBytes)} uncompressed
                      </div>
                    {/if}
                    <p class="breakdown-hint">
                      {#if compressionLevel === 'none'}
                        Rough estimate — actual file will be within ~5%. Images are the dominant cost.
                      {:else}
                        <strong>Compression estimates are particularly rough.</strong> Real savings depend on the source image quality and format — expect the final file anywhere from 30% smaller to 30% larger than this number. Check the exported file to see the real result.
                      {/if}
                    </p>
                  </div>
                {/if}
              {/if}

              {#if isEbook}
                <button class="export-btn-main" onclick={handleExportEpub}
                  disabled={exporting}>
                  {#if exporting}
                    <span class="export-spinner"></span> Exporting...
                  {:else}
                    <i class="bi bi-book"></i> Export EPUB
                  {/if}
                </button>
                <p class="export-hint">
                  Reflowable ebook for Kindle, Apple Books, Kobo, etc. Text reflows to fit the reader's screen and font preferences.
                </p>
              {:else}
                <button class="export-btn-main" onclick={handleExportPdf}
                  disabled={exporting || pageCount === 0}>
                  {#if exporting}
                    <span class="export-spinner"></span> Exporting...
                  {:else}
                    <i class="bi bi-file-earmark-pdf"></i> Export PDF
                  {/if}
                </button>
                <p class="export-hint">
                  Print-ready PDF with all fonts embedded. Uses the current profile's trim size, margins, and typography settings.
                </p>
              {/if}

              {#if exportError}
                <div class="export-error">
                  <i class="bi bi-exclamation-triangle"></i>
                  {exportError}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if designingPage}
    <PageContentEditor
      page={designingPage}
      profile={activeProfile}
      onsave={saveDesignedPage}
      oncancel={cancelDesignedPage} />
  {/if}

  {#if editingTocPage}
    <TocPageEditor
      page={editingTocPage}
      profile={activeProfile}
      onsave={saveTocPage}
      oncancel={cancelTocPage} />
  {/if}

  {#if editingMapPage}
    <MapPageEditor
      page={editingMapPage}
      profile={activeProfile}
      onsave={saveMapPage}
      oncancel={cancelMapPage} />
  {/if}

  {#if showAddPageModal}
    <div class="modal-backdrop" onclick={() => showAddPageModal = false}>
      <div class="modal-card add-page-modal" onclick={(e) => e.stopPropagation()}>
        <h3>Add Page</h3>
        <p class="add-page-hint">Choose a page type</p>
        <div class="page-type-options">
          {#each PAGE_TYPES as pt}
            <button class="page-type-option" onclick={() => addPageOfType(pt.id)}>
              <i class="bi {pt.icon}"></i>
              <div class="page-type-info">
                <span class="page-type-name">{pt.label}</span>
                <span class="page-type-desc">{pt.description}</span>
              </div>
            </button>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  {#if showCreateProfileModal}
    <div class="modal-backdrop" onclick={() => showCreateProfileModal = false}>
      <div class="modal-card" onclick={(e) => e.stopPropagation()}>
        <h3>New Profile</h3>
        <label class="modal-label">Name</label>
        <input class="modal-input" type="text" bind:value={newProfileName}
          placeholder="e.g. 5×8 Mass Market" autofocus
          onkeydown={(e) => { if (e.key === 'Enter') createProfile(); }} />

        <label class="modal-label">Duplicate from existing profile</label>
        <select class="modal-input" bind:value={newProfileDuplicateFrom}>
          <option value={null}>— Start blank —</option>
          {#each profiles as p}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>

        {#if !newProfileDuplicateFrom}
          <label class="modal-label">Target type</label>
          <div class="target-type-toggle">
            <button class="tt-btn" class:active={newProfileTargetType === 'print'}
              onclick={() => newProfileTargetType = 'print'}>Print</button>
            <button class="tt-btn" class:active={newProfileTargetType === 'ebook'}
              onclick={() => newProfileTargetType = 'ebook'}>Ebook</button>
          </div>
        {/if}

        <div class="modal-actions">
          <button class="modal-btn" onclick={() => showCreateProfileModal = false}>Cancel</button>
          <button class="modal-btn primary" onclick={createProfile} disabled={!newProfileName.trim()}>Create</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .format-loading {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 1rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .loader {
    width: 28px; height: 28px;
    border: 3px solid var(--iwe-border); border-top-color: var(--iwe-accent);
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Layout */
  .format-layout {
    display: flex; height: 100%; overflow: hidden;
  }

  .preview-column {
    flex: 1; display: flex; flex-direction: column; min-width: 0; overflow: hidden;
  }

  /* Preview area */
  .preview-area {
    flex: 1; overflow-y: auto; overflow-x: auto;
    background: #e8e4df;
    padding: 2rem;
    display: flex; justify-content: center;
  }
  .preview-scroll {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem 4px;
    padding-bottom: 2rem;
    justify-items: center;
  }
  /* First page is always recto (right) — push to column 2 */
  .preview-page-wrap:first-child { grid-column: 2; }
  /* Verso pages (even children = left page) align toward the spine (right edge) */
  .preview-page-wrap:nth-child(even) { justify-self: end; }
  /* Recto pages (odd children = right page) align toward the spine (left edge) */
  .preview-page-wrap:nth-child(odd) { justify-self: start; }
  .preview-page-wrap {
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
  }
  .preview-page-img {
    display: block;
    max-width: 100%;
    height: auto;
    box-shadow: 0 2px 12px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.06);
    background: #fff;
  }
  .preview-page-placeholder {
    background: #f5f3f0;
    border: 1px dashed rgba(0,0,0,0.12);
    box-shadow: 0 2px 12px rgba(0,0,0,0.06);
  }
  .page-number {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-align: center;
  }

  /* Ebook preview */
  .ebook-preview-wrap {
    display: flex; justify-content: center; align-items: flex-start;
    padding: 2rem; min-height: 100%;
  }
  .ebook-device {
    background: #1a1a1a;
    border-radius: 24px;
    padding: 16px 12px;
    box-shadow: 0 8px 40px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.05);
    transition: width 200ms ease;
  }
  .ebook-screen {
    background: #fff;
    border-radius: 4px;
    overflow-y: auto;
    transition: width 200ms ease, height 200ms ease;
  }

  /* Render states */
  .render-loading {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 0.8rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .render-error {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 0.6rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .render-error i { font-size: 1.5rem; color: #d97706; }
  .error-detail {
    font-size: 0.72rem; max-width: 500px; overflow-x: auto;
    background: var(--iwe-bg); padding: 0.5rem 0.8rem;
    border-radius: var(--iwe-radius-sm); border: 1px solid var(--iwe-border);
    white-space: pre-wrap; word-break: break-word;
  }
  .retry-btn {
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    padding: 0.35rem 0.9rem; border: 1px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm); background: none;
    color: var(--iwe-accent); cursor: pointer; transition: all 150ms;
  }
  .retry-btn:hover { background: var(--iwe-accent); color: #fff; }

  /* Timing bar */
  .timing-bar {
    flex-shrink: 0; display: flex; align-items: center; gap: 0.5rem;
    padding: 0.2rem 0.8rem; height: 24px;
    background: var(--iwe-bg-warm); border-top: 1px solid var(--iwe-border);
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); white-space: nowrap; overflow-x: auto;
  }
  .timing-sep { color: var(--iwe-border); }

  /* Resize handle */
  .resize-handle {
    width: 5px; flex-shrink: 0;
    cursor: col-resize;
    background: var(--iwe-border);
    position: relative;
    transition: background 150ms;
  }
  .resize-handle::after {
    content: '';
    position: absolute;
    top: 0; bottom: 0;
    left: -3px; right: -3px;
  }
  .resize-handle:hover,
  .resize-handle.active {
    background: var(--iwe-accent);
  }

  /* Sidebar */
  .format-sidebar {
    flex-shrink: 0;
    border-left: none;
    background: var(--iwe-bg-warm);
    display: flex; flex-direction: column;
    overflow: hidden;
  }

  .sidebar-section {
    padding: 0.75rem 0.9rem;
    border-bottom: 1px solid var(--iwe-border);
  }

  /* Profile selector */
  .profile-section { padding: 0.6rem 0.9rem; }
  .profile-row { display: flex; align-items: center; gap: 0.3rem; }
  .profile-select {
    flex: 1; padding: 0.3rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    min-width: 0;
  }
  .profile-select:focus { outline: none; border-color: var(--iwe-accent); }
  .profile-rename-input {
    flex: 1; padding: 0.3rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-accent); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); min-width: 0;
  }
  .profile-rename-input:focus { outline: none; }
  .profile-action-btn {
    border: 1px solid transparent; background: none;
    color: var(--iwe-text-muted); cursor: pointer;
    padding: 0.3rem 0.4rem; font-size: 0.85rem; line-height: 1;
    border-radius: var(--iwe-radius-sm); transition: all 120ms;
  }
  .profile-action-btn:hover:not(:disabled) {
    color: var(--iwe-accent); border-color: var(--iwe-accent);
  }
  .profile-action-btn:disabled { opacity: 0.3; cursor: not-allowed; }
  .profile-action-btn.danger:hover:not(:disabled) {
    color: #c0392b; border-color: #c0392b;
  }
  .confirm-delete {
    margin-top: 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text); display: flex; align-items: center; gap: 0.4rem;
  }
  .confirm-yes, .confirm-no {
    border: 1px solid var(--iwe-border); background: none;
    padding: 0.2rem 0.6rem; cursor: pointer;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    border-radius: var(--iwe-radius-sm);
  }
  .confirm-yes { color: #c0392b; border-color: #c0392b; }
  .confirm-yes:hover { background: #c0392b; color: #fff; }
  .confirm-no:hover { background: var(--iwe-bg-hover); }

  /* Modal */
  .modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.4);
    display: flex; align-items: center; justify-content: center;
    z-index: 1000;
  }
  .modal-card {
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius); padding: 1.4rem;
    min-width: 360px; max-width: 90vw;
    box-shadow: 0 20px 60px rgba(0,0,0,0.25);
    font-family: var(--iwe-font-ui);
  }
  .modal-card h3 {
    margin: 0 0 1rem 0;
    font-family: var(--iwe-font-prose); font-weight: 400;
    color: var(--iwe-text);
  }
  .modal-label {
    display: block; font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin: 0.8rem 0 0.3rem 0;
  }
  .modal-input {
    width: 100%; padding: 0.45rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
  }
  .modal-input:focus { outline: none; border-color: var(--iwe-accent); }
  .target-type-toggle {
    display: flex; gap: 0.4rem;
  }
  .tt-btn {
    flex: 1; padding: 0.45rem 0;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text-muted); cursor: pointer;
    transition: all 120ms;
  }
  .tt-btn.active {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.06); font-weight: 500;
  }
  .modal-actions {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    margin-top: 1.4rem;
  }
  .modal-btn {
    padding: 0.45rem 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 120ms;
  }
  .modal-btn:hover:not(:disabled) { background: var(--iwe-bg-hover); }
  .modal-btn.primary {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .modal-btn.primary:hover:not(:disabled) {
    background: #245a4f; border-color: #245a4f;
  }
  .modal-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  /* Profile pills (per-page inclusion) */
  .page-list-entry {
    margin-bottom: 0.3rem;
  }
  .profile-pills {
    display: flex; flex-wrap: wrap; gap: 4px;
    padding: 0.25rem 0.55rem 0.4rem 1.85rem;
  }
  .profile-pill {
    border: 1px solid var(--iwe-border); background: none;
    color: var(--iwe-text-muted);
    padding: 2px 8px; border-radius: 10px;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    cursor: pointer; transition: all 100ms;
    max-width: 100%; overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap;
  }
  .profile-pill.included {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
  }
  .profile-pill:not(.included):hover {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
  }
  .profile-pill.included:hover { opacity: 0.85; }
  .sidebar-label {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    margin-bottom: 0.4rem; display: block;
  }

  /* Mode tabs */
  .mode-section { padding: 0.5rem 0.9rem; }
  .mode-tabs {
    display: flex; gap: 2px;
    background: var(--iwe-bg); border-radius: var(--iwe-radius-sm);
    padding: 2px;
  }
  .mode-tab {
    flex: 1; display: flex; flex-direction: column; align-items: center; gap: 2px;
    padding: 0.4rem 0.2rem;
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    border: none; background: none; color: var(--iwe-text-muted);
    cursor: pointer; border-radius: var(--iwe-radius-sm);
    transition: all 150ms;
  }
  .mode-tab i { font-size: 0.9rem; }
  .mode-tab:hover { background: var(--iwe-bg-hover); color: var(--iwe-text); }
  .mode-tab.active {
    background: var(--iwe-bg-warm); color: var(--iwe-accent);
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }

  /* Mode content */
  .sidebar-mode-content {
    flex: 1; overflow-y: auto; overflow-x: hidden;
    padding: 0 0.9rem;
  }
  .mode-panel { padding: 0.2rem 0; }
  .page-list-panel { padding: 0; }
  .shell-placeholder {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); font-style: italic;
    text-align: center; padding: 1.5rem 0;
  }


  /* Custom mode sub-tab selector */
  .custom-selector-wrap {
    position: relative;
    margin-bottom: 0.6rem;
  }
  .custom-selector-btn {
    width: 100%;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text); cursor: pointer;
    transition: all 100ms;
  }
  .custom-selector-btn:hover { border-color: var(--iwe-accent); }
  .custom-selector-label { flex: 1; text-align: left; }
  .custom-selector-chevron {
    font-size: 0.7rem; color: var(--iwe-text-muted);
    transition: transform 150ms;
  }
  .custom-selector-chevron.open { transform: rotate(180deg); }
  .custom-selector-backdrop {
    position: fixed; inset: 0; z-index: 5;
    background: transparent; cursor: default;
  }
  .custom-selector-dropdown {
    position: absolute; top: calc(100% + 4px); left: 0; right: 0;
    z-index: 10;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    box-shadow: 0 8px 24px rgba(0,0,0,0.12);
    padding: 4px;
    max-height: 60vh; overflow-y: auto;
  }
  .custom-option {
    width: 100%;
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: none; background: none; color: var(--iwe-text); cursor: pointer;
    border-radius: var(--iwe-radius-sm); text-align: left;
    transition: background 100ms;
  }
  .custom-option:hover { background: var(--iwe-bg-hover); }
  .custom-option.active { color: var(--iwe-accent); font-weight: 500; }
  .custom-option i:first-child { width: 16px; text-align: center; }
  .custom-option-check {
    margin-left: auto; color: var(--iwe-accent);
  }

  /* Export panel */
  .export-panel { padding: 0.4rem 0; }
  .export-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 1rem 0; color: var(--iwe-text);
  }
  .export-info {
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.6rem 0.8rem;
    margin-bottom: 1rem;
  }
  .export-info-row {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.25rem 0;
  }
  .export-info-row + .export-info-row { border-top: 1px solid var(--iwe-border); }
  .export-info-label {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
  }
  .export-info-value {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text);
  }
  .export-info-muted {
    color: var(--iwe-text-faint);
    font-style: italic;
  }

  /* ---- Export size breakdown ---- */
  .export-size-breakdown {
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.7rem 0.8rem 0.75rem;
    margin-bottom: 1rem;
  }
  .breakdown-title {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.05em; font-weight: 600;
    margin-bottom: 0.45rem;
  }
  .breakdown-row {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.2rem 0;
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
  }
  .breakdown-dot {
    width: 9px; height: 9px; border-radius: 50%;
    flex-shrink: 0;
  }
  .breakdown-label {
    flex: 1;
    color: var(--iwe-text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .breakdown-value {
    color: var(--iwe-text-muted);
    font-variant-numeric: tabular-nums;
    font-size: 0.76rem;
  }
  .breakdown-total {
    margin-top: 0.35rem;
    padding-top: 0.45rem;
    border-top: 1px solid var(--iwe-border);
  }
  .breakdown-total .breakdown-label {
    font-weight: 600;
    color: var(--iwe-text);
    text-transform: uppercase;
    font-size: 0.72rem;
    letter-spacing: 0.05em;
  }
  .breakdown-total .breakdown-value {
    color: var(--iwe-text);
    font-weight: 600;
    font-size: 0.85rem;
  }
  .breakdown-dot-total {
    background: transparent;
    border: 1.5px solid var(--iwe-accent);
  }
  .breakdown-hint {
    margin: 0.55rem 0 0 0;
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); line-height: 1.45;
    font-style: italic;
  }
  .breakdown-hint strong {
    color: var(--iwe-text);
    font-weight: 600;
  }
  .breakdown-savings {
    display: flex; align-items: center; gap: 0.35rem;
    margin-top: 0.55rem;
    padding-top: 0.5rem;
    border-top: 1px dashed var(--iwe-border);
    font-family: var(--iwe-font-ui); font-size: 0.73rem;
    color: var(--iwe-accent);
    font-weight: 500;
  }
  .breakdown-title-badge {
    display: inline-block;
    margin-left: 0.4rem;
    padding: 0.05rem 0.35rem;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border);
    border-radius: 6px;
    font-size: 0.6rem;
    font-weight: 500;
    color: var(--iwe-text-muted);
    text-transform: none;
    letter-spacing: 0;
  }
  .estimate-badge {
    display: inline-block;
    margin-left: 0.3rem;
    padding: 0.05rem 0.35rem;
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: 6px;
    font-size: 0.6rem;
    color: var(--iwe-text-muted);
    cursor: help;
  }

  /* ---- Compression selector ---- */
  .export-compression {
    background: var(--iwe-bg-warm);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    padding: 0.7rem 0.8rem 0.75rem;
    margin-bottom: 1rem;
  }
  .compression-title {
    font-family: var(--iwe-font-ui); font-size: 0.68rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.05em; font-weight: 600;
    margin-bottom: 0.5rem;
  }
  .compression-options {
    display: flex; flex-direction: column; gap: 0.35rem;
  }
  .compression-option {
    display: flex; align-items: flex-start; gap: 0.55rem;
    padding: 0.5rem 0.55rem;
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    cursor: pointer;
    transition: border-color 120ms, background 120ms;
  }
  .compression-option:hover {
    border-color: var(--iwe-border-strong, var(--iwe-accent));
  }
  .compression-option.active {
    border-color: var(--iwe-accent);
    background: var(--iwe-bg);
    box-shadow: inset 0 0 0 1px var(--iwe-accent);
  }
  .compression-option input[type="radio"] {
    margin-top: 3px;
    accent-color: var(--iwe-accent);
    flex-shrink: 0;
  }
  .compression-option-body {
    display: flex; flex-direction: column; gap: 0.15rem;
    min-width: 0;
  }
  .compression-option-name {
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    font-weight: 500;
    color: var(--iwe-text);
  }
  .compression-option-desc {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
    line-height: 1.35;
  }
  .compression-hint {
    margin: 0.6rem 0 0 0;
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted);
    line-height: 1.45;
    font-style: italic;
  }

  .export-btn-main {
    width: 100%;
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    padding: 0.75rem 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.95rem; font-weight: 500;
    background: var(--iwe-accent); border: 1px solid var(--iwe-accent);
    color: #fff; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .export-btn-main:hover:not(:disabled) { background: #245a4f; }
  .export-btn-main:disabled { opacity: 0.5; cursor: not-allowed; }
  .export-buttons {
    display: flex; flex-direction: column; gap: 0.5rem;
  }
  .export-btn-main i { font-size: 1.1rem; }
  .export-btn-secondary {
    width: 100%;
    display: flex; align-items: center; justify-content: center; gap: 0.5rem;
    padding: 0.65rem 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.9rem; font-weight: 500;
    background: var(--iwe-bg); border: 1px solid var(--iwe-accent);
    color: var(--iwe-accent); border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: all 120ms;
  }
  .export-btn-secondary:hover:not(:disabled) { background: rgba(45, 106, 94, 0.06); }
  .export-btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }
  .export-btn-secondary i { font-size: 1rem; }
  .export-hints { margin-top: 1rem; }
  .export-spinner {
    width: 16px; height: 16px;
    border: 2px solid rgba(255,255,255,0.3); border-top-color: #fff;
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  .export-error {
    margin-top: 0.7rem;
    padding: 0.5rem 0.7rem;
    background: rgba(192, 57, 43, 0.08);
    border: 1px solid rgba(192, 57, 43, 0.2);
    border-radius: var(--iwe-radius-sm);
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: #c0392b;
    display: flex; align-items: flex-start; gap: 0.4rem;
  }
  .export-hint {
    margin-top: 1rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); line-height: 1.5;
  }

  /* Tag bar */
  .tag-bar {
    display: flex; flex-wrap: wrap; gap: 4px;
    padding: 0.5rem 0 0.5rem 0;
    border-bottom: 1px solid var(--iwe-border);
    margin-bottom: 0.4rem;
  }
  .tag-pill {
    display: inline-flex; align-items: center; gap: 3px;
    border: 1px solid var(--iwe-border); background: var(--iwe-bg);
    color: var(--iwe-text);
    padding: 3px 10px; border-radius: 11px;
    font-family: var(--iwe-font-ui); font-size: 0.8rem;
    cursor: pointer; transition: all 100ms;
  }
  .tag-pill:hover:not(.used) {
    border-color: var(--iwe-accent); color: var(--iwe-accent);
  }
  .tag-pill.armed {
    background: var(--iwe-accent); border-color: var(--iwe-accent); color: #fff;
    box-shadow: 0 0 0 2px rgba(45, 106, 94, 0.2);
  }
  .tag-pill.used {
    background: var(--iwe-bg-hover); color: var(--iwe-text-muted);
    cursor: default; opacity: 0.7;
  }
  .tag-x {
    display: inline-flex; align-items: center; justify-content: center;
    width: 14px; height: 14px; border-radius: 50%;
    color: var(--iwe-text-muted); font-size: 0.85rem; line-height: 1;
    cursor: pointer; transition: all 100ms;
  }
  .tag-x:hover {
    background: #c0392b; color: #fff;
  }
  .armed-hint {
    font-family: var(--iwe-font-ui); font-size: 0.7rem;
    color: var(--iwe-text-muted); padding: 0.3rem 0.4rem;
    background: rgba(45, 106, 94, 0.06);
    border-left: 2px solid var(--iwe-accent);
    border-radius: var(--iwe-radius-sm);
    margin-bottom: 0.5rem;
  }
  .armed-hint kbd {
    font-family: monospace; font-size: 0.65rem;
    background: var(--iwe-bg); border: 1px solid var(--iwe-border);
    padding: 0 4px; border-radius: 3px;
  }

  /* Page list — when a tag is armed, page items become click targets */
  .page-list-item.armed-target {
    cursor: copy;
  }
  .page-list-item.armed-target:hover {
    background: rgba(45, 106, 94, 0.12);
    box-shadow: inset 0 -2px 0 var(--iwe-accent);
  }

  /* Page list */
  .page-group-label {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
    padding: 0.6rem 0 0.3rem 0;
    display: flex; align-items: center; justify-content: space-between;
  }
  .add-page-btn {
    border: none; background: none; color: var(--iwe-accent);
    cursor: pointer; padding: 0.1rem 0.35rem; font-size: 1.1rem;
    line-height: 1; border-radius: var(--iwe-radius-sm);
    transition: background 100ms;
  }
  .add-page-btn:hover { background: var(--iwe-bg-hover); }

  .dnd-zone { min-height: 4px; }

  .page-list-item {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.55rem; border-radius: var(--iwe-radius-sm);
    cursor: pointer; transition: background 100ms;
    font-family: var(--iwe-font-ui); font-size: 0.92rem;
  }
  .page-list-item:hover { background: var(--iwe-bg-hover); }

  .format-page-item .drag-handle {
    color: var(--iwe-text-muted); font-size: 1.05rem;
    cursor: grab; opacity: 0.5;
  }
  .format-page-item:hover .drag-handle { opacity: 1; }

  .page-item-title {
    flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--iwe-text);
  }
  .page-item-role {
    font-size: 0.72rem; color: var(--iwe-text-muted);
    white-space: nowrap;
  }
  .page-item-edit, .page-item-delete {
    border: none; background: none; color: var(--iwe-text-muted);
    cursor: pointer; padding: 0.15rem 0.3rem; font-size: 1rem;
    opacity: 0; transition: opacity 100ms, color 100ms;
  }
  .page-list-item:hover .page-item-edit,
  .page-list-item:hover .page-item-delete { opacity: 0.75; }
  .page-item-edit:hover { color: var(--iwe-accent); opacity: 1 !important; }
  .page-item-delete:hover { color: #c0392b; opacity: 1 !important; }
  .page-item-input {
    flex: 1; min-width: 0;
    border: 1px solid var(--iwe-accent); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    font-family: var(--iwe-font-ui); font-size: 0.92rem;
    padding: 0.2rem 0.4rem;
  }
  .page-item-input:focus { outline: none; }

  /* Chapter items are visually distinct */
  .chapter-item {
    padding-left: 1.6rem;
    opacity: 0.75;
    cursor: pointer;
  }
  .chapter-item .page-item-title {
    font-style: italic;
  }

  /* Ebook metadata tag badge on page items */
  .page-item-ebook-tag {
    font-size: 0.6rem; text-transform: uppercase;
    letter-spacing: 0.03em; font-weight: 600;
    background: rgba(45, 106, 94, 0.1); color: var(--iwe-accent);
    padding: 1px 6px; border-radius: 6px;
    white-space: nowrap;
  }

  /* Add page modal */
  .add-page-modal {
    max-width: 380px;
  }
  .add-page-hint {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); margin: 0 0 0.8rem;
  }
  .page-type-options {
    display: flex; flex-direction: column; gap: 6px;
  }
  .page-type-option {
    display: flex; align-items: center; gap: 0.75rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    padding: 0.65rem 0.85rem;
    cursor: pointer; transition: all 100ms;
    text-align: left;
    font-family: var(--iwe-font-ui);
  }
  .page-type-option:hover {
    border-color: var(--iwe-accent);
    background: rgba(45, 106, 94, 0.04);
  }
  .page-type-option i {
    font-size: 1.2rem; color: var(--iwe-accent);
    flex-shrink: 0;
  }
  .page-type-info {
    display: flex; flex-direction: column; gap: 1px;
  }
  .page-type-name {
    font-size: 0.88rem; font-weight: 600;
    color: var(--iwe-text);
  }
  .page-type-desc {
    font-size: 0.72rem;
    color: var(--iwe-text-muted);
  }

</style>
