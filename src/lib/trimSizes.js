// Master trim size catalog with platform compatibility.
// Sources: KDP, IngramSpark, Lulu, B&N Press, Draft2Digital, BookBaby (as of 2025).
// Dimensions in inches. Platform flags: kdp, kdpHc, is, lulu, bn, d2d, bb

export const PLATFORMS = [
  { id: 'kdp',   label: 'KDP',          short: 'KDP' },
  { id: 'kdpHc', label: 'KDP Hardcover', short: 'KDP HC' },
  { id: 'is',    label: 'IngramSpark',  short: 'IS' },
  { id: 'lulu',  label: 'Lulu',         short: 'Lulu' },
  { id: 'bn',    label: 'B&N Press',    short: 'B&N' },
  { id: 'd2d',   label: 'Draft2Digital', short: 'D2D' },
  { id: 'bb',    label: 'BookBaby',     short: 'BB' },
];

export const TRIM_CATEGORIES = [
  {
    label: 'Fiction / Standard',
    hint: 'Most common for novels and general fiction',
    sizes: [
      { w: 5,    h: 8,    label: '5 × 8',       kdp: true, kdpHc: true, is: true, lulu: true, bn: true, d2d: true, bb: true },
      { w: 5.25, h: 8,    label: '5.25 × 8',    kdp: true, is: true, bn: true, d2d: true },
      { w: 5.5,  h: 8.5,  label: '5.5 × 8.5',   kdp: true, kdpHc: true, is: true, lulu: true, bn: true, d2d: true, bb: true },
      { w: 6,    h: 9,    label: '6 × 9',        kdp: true, kdpHc: true, is: true, lulu: true, bn: true, d2d: true, bb: true },
    ],
  },
  {
    label: 'Additional Trade',
    hint: 'Less common trade sizes for specific markets',
    sizes: [
      { w: 5.06, h: 7.81, label: '5.06 × 7.81', kdp: true, is: true, bn: true },
      { w: 5.5,  h: 8.25, label: '5.5 × 8.25',  bn: true },
      { w: 6.14, h: 9.21, label: '6.14 × 9.21', kdp: true, kdpHc: true, is: true, lulu: true, d2d: true },
      { w: 6.69, h: 9.61, label: '6.69 × 9.61', kdp: true, kdpHc: true, is: true, d2d: true },
    ],
  },
  {
    label: 'International',
    hint: 'Standard metric sizes used outside the US',
    sizes: [
      { w: 5.83, h: 8.27, label: '5.83 × 8.27 (A5)',  is: true, lulu: true, bn: true },
      { w: 5.31, h: 8.46, label: '5.31 × 8.46 (Demy)', },
      { w: 4.72, h: 7.48, label: '4.72 × 7.48 (B-format)', is: true },
      { w: 4.92, h: 7.48, label: '4.92 × 7.48',        },
      { w: 8.27, h: 11.69, label: '8.27 × 11.69 (A4)', is: true, lulu: true, bn: true },
    ],
  },
  {
    label: 'Mass Market',
    hint: 'Smaller paperbacks — romance, thriller, mystery',
    sizes: [
      { w: 4.25, h: 6.875, label: '4.25 × 6.875', lulu: true, bb: true },
      { w: 4.25, h: 7,     label: '4.25 × 7',      is: true, bn: true },
      { w: 4.37, h: 7,     label: '4.37 × 7',      is: true, bn: true },
      { w: 4,    h: 6,     label: '4 × 6',          is: true, bn: true },
      { w: 4.12, h: 6.75,  label: '4.12 × 6.75',   },
    ],
  },
  {
    label: 'Large Format / Nonfiction',
    hint: 'Textbooks, cookbooks, workbooks, manuals',
    sizes: [
      { w: 7,    h: 10,   label: '7 × 10',       kdp: true, kdpHc: true, is: true, lulu: true, bn: true, bb: true },
      { w: 7.44, h: 9.69, label: '7.44 × 9.69',  kdp: true, kdpHc: true, is: true, lulu: true, d2d: true },
      { w: 7.5,  h: 9.25, label: '7.5 × 9.25',   kdp: true, kdpHc: true, is: true, bn: true, d2d: true },
      { w: 8,    h: 10,   label: '8 × 10',        kdp: true, kdpHc: true, is: true, bn: true, d2d: true, bb: true },
      { w: 8.25, h: 11,   label: '8.25 × 11',     is: true, bn: true },
      { w: 8.5,  h: 11,   label: '8.5 × 11',      kdp: true, kdpHc: true, is: true, lulu: true, bn: true, d2d: true, bb: true },
    ],
  },
  {
    label: 'Square / Children\'s',
    hint: 'Picture books, art books, coffee table books',
    sizes: [
      { w: 7.5,  h: 7.5,  label: '7.5 × 7.5',   lulu: true, bb: true },
      { w: 8,    h: 8,    label: '8 × 8',         is: true },
      { w: 8.25, h: 8.25, label: '8.25 × 8.25',  kdp: true },
      { w: 8.5,  h: 8.5,  label: '8.5 × 8.5',    kdp: true, is: true, lulu: true, bb: true },
      { w: 10,   h: 10,   label: '10 × 10',       bb: true },
    ],
  },
  {
    label: 'Landscape',
    hint: 'Wide-format for photo books and children\'s books',
    sizes: [
      { w: 9,    h: 7,    label: '9 × 7',         is: true, lulu: true, bb: true },
      { w: 10,   h: 8,    label: '10 × 8',        is: true },
      { w: 11,   h: 8.5,  label: '11 × 8.5',      is: true, bn: true, bb: true },
    ],
  },
];

// Flat list of all sizes for search/filter
export const ALL_SIZES = TRIM_CATEGORIES.flatMap(cat =>
  cat.sizes.map(s => ({ ...s, category: cat.label }))
);

/**
 * Find a size entry by dimensions (within tolerance).
 */
export function findSize(w, h, tolerance = 0.05) {
  return ALL_SIZES.find(
    s => Math.abs(s.w - w) < tolerance && Math.abs(s.h - h) < tolerance
  );
}

/**
 * Get the list of platform IDs that support a given size.
 */
export function supportedPlatforms(size) {
  return PLATFORMS.filter(p => size[p.id]).map(p => p.id);
}
