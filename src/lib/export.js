import { Document, Packer, Paragraph, TextRun, HeadingLevel, AlignmentType, BorderStyle } from 'docx';


import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

/**
 * Strip HTML to plain text
 */
function htmlToPlain(html) {
  return html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<\/h[1-3]>/gi, '\n\n')
    .replace(/<hr\s*\/?>/gi, '\n* * *\n')
    .replace(/<\/li>/gi, '\n')
    .replace(/<\/blockquote>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/**
 * Parse HTML content into docx paragraph elements
 */
function htmlToDocxParagraphs(html) {
  const paragraphs = [];

  // Split by block-level tags
  const blocks = html.split(/<\/(?:p|h[1-3]|li|blockquote)>/gi);

  for (const block of blocks) {
    const trimmed = block.trim();
    if (!trimmed) continue;

    // Detect heading level
    const h1Match = trimmed.match(/<h1[^>]*>/i);
    const h2Match = trimmed.match(/<h2[^>]*>/i);
    const h3Match = trimmed.match(/<h3[^>]*>/i);
    const hrMatch = trimmed.match(/<hr[^>]*>/i);
    const bqMatch = trimmed.match(/<blockquote[^>]*>/i);

    if (hrMatch) {
      paragraphs.push(new Paragraph({
        children: [new TextRun({ text: '* * *' })],
        alignment: AlignmentType.CENTER,
        spacing: { before: 400, after: 400 },
      }));
      continue;
    }

    // Strip remaining tags to get text, preserving inline formatting
    const runs = parseInlineFormatting(trimmed);

    if (runs.length === 0) continue;

    const heading = h1Match ? HeadingLevel.HEADING_1
      : h2Match ? HeadingLevel.HEADING_2
      : h3Match ? HeadingLevel.HEADING_3
      : undefined;

    // Check text alignment
    const alignMatch = trimmed.match(/text-align:\s*(center|right|justify)/i);
    const alignment = alignMatch
      ? alignMatch[1] === 'center' ? AlignmentType.CENTER
        : alignMatch[1] === 'right' ? AlignmentType.RIGHT
        : alignMatch[1] === 'justify' ? AlignmentType.JUSTIFIED
        : undefined
      : undefined;

    const paraOpts = { children: runs };
    if (heading) paraOpts.heading = heading;
    if (alignment) paraOpts.alignment = alignment;
    if (bqMatch) {
      paraOpts.indent = { left: 720 }; // 0.5 inch
      paraOpts.border = {
        left: { style: BorderStyle.SINGLE, size: 6, color: '999999' },
      };
    }

    paragraphs.push(new Paragraph(paraOpts));
  }

  return paragraphs;
}

/**
 * Parse inline HTML formatting (bold, italic, underline, strikethrough) into TextRuns
 */
function parseInlineFormatting(html) {
  // Strip block-level opening tags
  let text = html.replace(/<(?:p|h[1-3]|li|blockquote|div)[^>]*>/gi, '');

  const runs = [];
  // Simple approach: strip tags and detect formatting regions
  // For now, just extract text with basic bold/italic detection

  const segments = [];
  let current = { text: '', bold: false, italic: false, underline: false, strike: false };
  let tagStack = [];

  const tagRe = /<\/?(?:strong|b|em|i|u|s|strike|del|sup|sub|br|span)[^>]*>|[^<]+/gi;
  let match;

  while ((match = tagRe.exec(text)) !== null) {
    const token = match[0];

    if (token === '<br>' || token === '<br/>' || token === '<br />') {
      if (current.text) { segments.push({ ...current }); current = { ...current, text: '' }; }
      segments.push({ text: '\n', break: true });
    } else if (token.startsWith('<')) {
      if (current.text) { segments.push({ ...current }); current = { ...current, text: '' }; }

      const isClose = token.startsWith('</');
      const tag = token.replace(/<\/?/, '').replace(/[^a-z]/gi, '').toLowerCase();

      if (isClose) {
        if (tag === 'strong' || tag === 'b') current.bold = false;
        else if (tag === 'em' || tag === 'i') current.italic = false;
        else if (tag === 'u') current.underline = false;
        else if (tag === 's' || tag === 'strike' || tag === 'del') current.strike = false;
      } else {
        if (tag === 'strong' || tag === 'b') current.bold = true;
        else if (tag === 'em' || tag === 'i') current.italic = true;
        else if (tag === 'u') current.underline = true;
        else if (tag === 's' || tag === 'strike' || tag === 'del') current.strike = true;
      }
    } else {
      // Text content — decode entities
      current.text += token
        .replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>')
        .replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&nbsp;/g, ' ');
    }
  }
  if (current.text) segments.push({ ...current });

  for (const seg of segments) {
    if (seg.break) {
      runs.push(new TextRun({ break: 1 }));
    } else if (seg.text) {
      runs.push(new TextRun({
        text: seg.text,
        bold: seg.bold || false,
        italics: seg.italic || false,
        underline: seg.underline ? {} : undefined,
        strike: seg.strike || false,
        font: 'Times New Roman',
        size: 24, // 12pt
      }));
    }
  }

  return runs;
}

/**
 * Export manuscript as DOCX
 */
export async function exportDocx(chapters, projectTitle) {
  const sections = [];

  for (const chapter of chapters) {
    const paragraphs = [
      new Paragraph({
        children: [new TextRun({ text: chapter.title, bold: true, font: 'Times New Roman', size: 32 })],
        heading: HeadingLevel.HEADING_1,
        spacing: { after: 400 },
        pageBreakBefore: sections.length > 0,
      }),
      ...htmlToDocxParagraphs(chapter.content),
    ];

    sections.push({ children: paragraphs });
  }

  const doc = new Document({
    creator: 'IWE - Integrated Writing Environment',
    title: projectTitle,
    styles: {
      paragraphStyles: [
        {
          id: 'Heading1',
          name: 'Heading 1',
          run: { font: 'Times New Roman', size: 32, bold: true, color: '000000' },
          paragraph: { spacing: { before: 240, after: 120 } },
        },
        {
          id: 'Heading2',
          name: 'Heading 2',
          run: { font: 'Times New Roman', size: 28, bold: true, color: '000000' },
          paragraph: { spacing: { before: 200, after: 100 } },
        },
        {
          id: 'Heading3',
          name: 'Heading 3',
          run: { font: 'Times New Roman', size: 24, bold: true, italics: true, color: '000000' },
          paragraph: { spacing: { before: 160, after: 80 } },
        },
      ],
    },
    sections: sections.length > 0 ? sections : [{ children: [new Paragraph({ children: [new TextRun('')] })] }],
  });

  const blob = await Packer.toBlob(doc);
  const arrayBuffer = await blob.arrayBuffer();
  const uint8 = new Uint8Array(arrayBuffer);

  const path = await save({
    title: 'Export as DOCX',
    defaultPath: `${projectTitle}.docx`,
    filters: [{ name: 'Word Document', extensions: ['docx'] }],
  });

  if (path) {
    await writeFile(path, uint8);
    return path;
  }
  return null;
}

/**
 * Export manuscript as plain text
 */
export async function exportTxt(chapters, projectTitle) {
  let text = '';
  for (const chapter of chapters) {
    text += `${chapter.title}\n${'='.repeat(chapter.title.length)}\n\n`;
    text += htmlToPlain(chapter.content);
    text += '\n\n\n';
  }

  const path = await save({
    title: 'Export as Text',
    defaultPath: `${projectTitle}.txt`,
    filters: [{ name: 'Text File', extensions: ['txt'] }],
  });

  if (path) {
    const encoder = new TextEncoder();
    await writeFile(path, encoder.encode(text));
    return path;
  }
  return null;
}

/**
 * Export manuscript as HTML
 */
export async function exportHtml(chapters, projectTitle) {
  let html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>${projectTitle}</title>
  <style>
    body { font-family: 'Times New Roman', Georgia, serif; max-width: 700px; margin: 2rem auto; padding: 0 1rem; line-height: 1.8; color: #222; }
    h1 { font-size: 1.8rem; margin: 3rem 0 1rem; page-break-before: always; }
    h2 { font-size: 1.4rem; margin: 2rem 0 0.8rem; }
    h3 { font-size: 1.1rem; margin: 1.5rem 0 0.5rem; font-style: italic; }
    blockquote { border-left: 3px solid #ccc; padding-left: 1rem; margin: 1rem 0; color: #555; font-style: italic; }
    hr { border: none; text-align: center; margin: 2rem 0; }
    hr::after { content: '* * *'; letter-spacing: 0.5em; color: #999; }
  </style>
</head>
<body>
`;

  for (const chapter of chapters) {
    html += `<h1>${chapter.title}</h1>\n${chapter.content}\n\n`;
  }

  html += '</body>\n</html>';

  const path = await save({
    title: 'Export as HTML',
    defaultPath: `${projectTitle}.html`,
    filters: [{ name: 'HTML File', extensions: ['html'] }],
  });

  if (path) {
    const encoder = new TextEncoder();
    await writeFile(path, encoder.encode(html));
    return path;
  }
  return null;
}

/**
 * Export as PDF — generated directly in Rust, no print dialog.
 */
export async function exportPdf(chapters, projectTitle, format = 'a4') {
  const { invoke } = await import('@tauri-apps/api/core');

  const path = await save({
    title: `Export as PDF (${format === 'book' ? 'Book 5×8' : 'A4'})`,
    defaultPath: `${projectTitle}.pdf`,
    filters: [{ name: 'PDF Document', extensions: ['pdf'] }],
  });

  if (path) {
    await invoke('export_pdf', { path, title: projectTitle, format });
    return path;
  }
  return null;
}
