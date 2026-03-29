<script>
  import { onMount } from 'svelte';
  import { getChapters } from '$lib/db.js';

  let ready = $state(false);

  onMount(async () => {
    const params = new URLSearchParams(window.location.search);
    const format = params.get('format') || 'a4';
    const projectTitle = params.get('title') || 'Untitled';

    try {
      const chapters = await getChapters();
      if (chapters.length === 0) { ready = true; return; }

      const html = buildPrintHtml(chapters, projectTitle, format);
      document.open();
      document.write(html);
      document.close();

      // Add floating print button
      const btn = document.createElement('div');
      btn.innerHTML = `
        <button onclick="this.parentElement.style.display='none'; window.print();" style="
          font-family: system-ui, sans-serif; font-size: 14px;
          padding: 10px 24px; background: #2d6a5e; color: white;
          border: none; border-radius: 6px; cursor: pointer;
          box-shadow: 0 4px 12px rgba(0,0,0,0.15);
        ">Save as PDF / Print</button>
        <p style="font-size: 12px; color: #999; margin: 8px 0 0; font-family: system-ui;">
          Use "Save as PDF" in the print dialog
        </p>
      `;
      btn.style.cssText = `
        position: fixed; top: 20px; right: 20px; z-index: 10000;
        background: white; padding: 16px; border-radius: 8px;
        border: 1px solid #e5e1da; box-shadow: 0 4px 20px rgba(0,0,0,0.1);
        text-align: center;
      `;
      btn.className = 'no-print';
      document.body.appendChild(btn);
    } catch (e) {
      console.error('Print failed:', e);
      ready = true;
    }
  });

  function buildPrintHtml(chapters, projectTitle, format) {
    const isBook = format === 'book';
    const pageWidth = isBook ? '5in' : '210mm';
    const pageHeight = isBook ? '8in' : '297mm';
    const margin = isBook ? '0.6in 0.5in' : '1in';
    const fontSize = isBook ? '11pt' : '12pt';
    const lineHeight = isBook ? '1.6' : '1.8';
    const fontFamily = isBook
      ? "'Georgia', 'Libre Baskerville', serif"
      : "'Times New Roman', Georgia, serif";

    let html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>${projectTitle}</title>
<style>
  @page { size: ${pageWidth} ${pageHeight}; margin: ${margin}; }
  * { box-sizing: border-box; }
  body {
    font-family: ${fontFamily}; font-size: ${fontSize};
    line-height: ${lineHeight}; color: #1a1a1a; margin: 0; padding: 0;
  }
  .title-page {
    display: flex; flex-direction: column; align-items: center;
    justify-content: center; height: 100vh; text-align: center;
    page-break-after: always;
  }
  .title-page h1 {
    font-size: ${isBook ? '2rem' : '2.5rem'}; font-weight: 400;
    letter-spacing: 0.05em; margin: 0;
  }
  .title-page .rule { width: 60px; height: 1px; background: #666; margin: 1.5rem 0; }
  .chapter { page-break-before: always; }
  .chapter:first-of-type { page-break-before: auto; }
  .chapter-title {
    font-size: ${isBook ? '1rem' : '1.6rem'};
    font-weight: ${isBook ? '400' : '700'};
    text-align: center;
    margin: ${isBook ? '2rem 0 1.5rem' : '3rem 0 2rem'};
    ${isBook ? 'letter-spacing: 0.05em; text-transform: uppercase;' : ''}
  }
  p { margin: 0 0 0.8em; ${isBook ? 'text-indent: 1.5em; margin: 0;' : ''} }
  ${isBook ? 'p:first-of-type, h1+p, h2+p, h3+p, hr+p { text-indent: 0; }' : ''}
  h2 { font-size: 1.3em; margin: 1.5em 0 0.5em; }
  h3 { font-size: 1.1em; margin: 1.2em 0 0.4em; font-style: italic; }
  blockquote { border-left: 2px solid #999; padding-left: 1em; margin: 1em 0; color: #444; font-style: italic; }
  hr { border: none; text-align: center; margin: ${isBook ? '1.5em 0' : '2em 0'}; }
  hr::after { content: '\\2022  \\2022  \\2022'; color: #999; letter-spacing: 0.3em; }
  .no-print { }
  @media print { .no-print { display: none !important; } body { -webkit-print-color-adjust: exact; } }
</style>
</head>
<body>
<div class="title-page"><h1>${projectTitle}</h1><div class="rule"></div></div>
`;
    for (const ch of chapters) {
      html += `<div class="chapter"><div class="chapter-title">${ch.title}</div>${ch.content}</div>\n`;
    }
    html += '</body></html>';
    return html;
  }
</script>

{#if ready}
  <div style="display: flex; align-items: center; justify-content: center; height: 100vh; font-family: system-ui; color: #999;">
    No content to print. Close this window and try again.
  </div>
{/if}

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }
</style>
