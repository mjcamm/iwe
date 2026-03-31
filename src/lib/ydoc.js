import * as Y from 'yjs';

/**
 * Create a Y.Doc for a chapter from stored state bytes.
 * @param {number[]|Uint8Array|null} stateBytes - Y.Doc state from Rust/SQLite
 * @returns {{ doc: Y.Doc, xmlFragment: Y.XmlFragment }}
 */
export function createChapterDoc(stateBytes) {
  const doc = new Y.Doc();
  if (stateBytes && stateBytes.length > 0) {
    const bytes = stateBytes instanceof Uint8Array ? stateBytes : new Uint8Array(stateBytes);
    Y.applyUpdate(doc, bytes);
  }
  const xmlFragment = doc.getXmlFragment('prosemirror');
  return { doc, xmlFragment };
}

/**
 * Encode full Y.Doc state as Uint8Array for storage.
 * @param {Y.Doc} doc
 * @returns {Uint8Array}
 */
export function encodeDoc(doc) {
  return Y.encodeStateAsUpdate(doc);
}

/**
 * Destroy a Y.Doc to free memory.
 * @param {Y.Doc} doc
 */
export function destroyDoc(doc) {
  if (doc) {
    doc.destroy();
  }
}
