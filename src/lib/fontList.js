// Cached system font list. Fetched once on first use, shared across all FontPicker
// instances. Components call ensureFontList() to get the cached array.

import { listSystemFonts } from './db.js';

let cachedFonts = null;
let fetchPromise = null;

export async function ensureFontList() {
  if (cachedFonts) return cachedFonts;
  if (fetchPromise) return fetchPromise;
  fetchPromise = listSystemFonts()
    .then(list => {
      cachedFonts = list;
      return list;
    })
    .catch(e => {
      console.error('[fonts] failed to load system fonts:', e);
      cachedFonts = [];
      return cachedFonts;
    });
  return fetchPromise;
}
