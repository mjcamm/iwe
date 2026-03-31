use yrs::{Doc, GetString, ReadTxn, Transact, Update, StateVector, XmlFragment};
use yrs::types::xml::{XmlFragmentRef, XmlOut};
use yrs::updates::decoder::Decode;

/// Load a Y.Doc from encoded state bytes (from SQLite BLOB).
pub fn load_doc(state_bytes: &[u8]) -> Result<Doc, String> {
    let doc = Doc::new();
    if !state_bytes.is_empty() {
        let update = Update::decode_v1(state_bytes)
            .map_err(|e| format!("Failed to decode Y.Doc state: {}", e))?;
        let mut txn = doc.transact_mut();
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply Y.Doc update: {:?}", e))?;
    }
    Ok(doc)
}

/// Encode full Y.Doc state to bytes for storage.
pub fn encode_doc(doc: &Doc) -> Vec<u8> {
    let txn = doc.transact();
    txn.encode_state_as_update_v1(&StateVector::default())
}

/// Extract plain text from Y.Doc's prosemirror XmlFragment.
/// Walks all children depth-first, concatenating text from XmlText nodes.
/// NO separators between blocks — matches frontend buildTextMap() behavior.
pub fn extract_plain_text(doc: &Doc) -> String {
    let txn = doc.transact();
    match txn.get_xml_fragment("prosemirror") {
        Some(fragment) => {
            let mut text = String::new();
            walk_fragment(&txn, &fragment, &mut text);
            text
        }
        None => String::new(),
    }
}

/// Extract text with paragraph breaks (for PDF/plain-text export).
/// Inserts \n\n between block-level elements.
pub fn extract_text_with_breaks(doc: &Doc) -> String {
    let txn = doc.transact();
    match txn.get_xml_fragment("prosemirror") {
        Some(fragment) => {
            let mut text = String::new();
            let mut first = true;
            for child in fragment.children(&txn) {
                if !first && !text.is_empty() {
                    text.push_str("\n\n");
                }
                first = false;
                walk_xml_out(&txn, &child, &mut text);
            }
            text
        }
        None => String::new(),
    }
}

fn walk_fragment<T: ReadTxn>(txn: &T, fragment: &XmlFragmentRef, out: &mut String) {
    for child in fragment.children(txn) {
        walk_xml_out(txn, &child, out);
    }
}

fn walk_xml_out<T: ReadTxn>(txn: &T, node: &XmlOut, out: &mut String) {
    match node {
        XmlOut::Element(el) => {
            for inner in el.children(txn) {
                walk_xml_out(txn, &inner, out);
            }
        }
        XmlOut::Text(txt) => {
            out.push_str(&txt.get_string(txn));
        }
        XmlOut::Fragment(frag) => {
            walk_fragment(txn, frag, out);
        }
    }
}

/// Count words in a Y.Doc chapter.
pub fn word_count(doc: &Doc) -> usize {
    let text = extract_plain_text(doc);
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .count()
}
