// Professional margin recommendations per trim size.
// Values are in inches and follow industry best practices:
// - Bottom > Top (optical center convention)
// - Inside > Outside (gutter absorbs into binding)
// - Comfortable margins (well above KDP/IngramSpark minimums)

export const MARGIN_PRESETS = [
  // Fiction paperbacks
  { w: 6,    h: 9,    label: '6×9',       top: 1.0,   bottom: 0.875, outside: 0.625, inside: 0.875 },
  { w: 5.5,  h: 8.5,  label: '5.5×8.5',   top: 0.825, bottom: 0.825, outside: 0.625, inside: 0.75 },
  { w: 5,    h: 8,    label: '5×8',       top: 0.75,  bottom: 0.875, outside: 0.625, inside: 0.75 },
  // International
  { w: 5.83, h: 8.27, label: 'A5',        top: 0.75,  bottom: 0.875, outside: 0.625, inside: 0.75 },
  // Large formats
  { w: 8.5,  h: 11,   label: 'US Letter', top: 1.0,   bottom: 1.0,   outside: 0.75,  inside: 1.0 },
  // Ebook placeholders — ebooks don't really have fixed margins, but the preview still needs values
  { w: 4.5,  h: 7.2,  label: 'Kindle',    top: 0.5,   bottom: 0.5,   outside: 0.4,   inside: 0.4 },
  { w: 5,    h: 7.5,  label: 'EPUB',      top: 0.5,   bottom: 0.5,   outside: 0.4,   inside: 0.4 },
];

/**
 * Look up recommended margins for a given trim size.
 * Uses exact-match first (within 0.05″ tolerance); falls back to proportional
 * scaling based on page height if no preset matches.
 */
export function getRecommendedMargins(widthIn, heightIn) {
  for (const p of MARGIN_PRESETS) {
    if (Math.abs(p.w - widthIn) < 0.05 && Math.abs(p.h - heightIn) < 0.05) {
      return { top: p.top, bottom: p.bottom, outside: p.outside, inside: p.inside };
    }
  }
  // Fallback: scale a 6×9 baseline by page height
  const scale = heightIn / 9;
  return {
    top:     1.0   * scale,
    bottom:  0.875 * scale,
    outside: 0.625 * scale,
    inside:  0.875 * scale,
  };
}

/**
 * Recommended gutter adjustment based on page count.
 * Rule of thumb: +0.125″ per 100 pages beyond 150.
 */
export function gutterForPageCount(baseGutterIn, pageCount) {
  if (pageCount <= 150) return baseGutterIn;
  const extra = Math.floor((pageCount - 150) / 100) * 0.125;
  return baseGutterIn + extra;
}
