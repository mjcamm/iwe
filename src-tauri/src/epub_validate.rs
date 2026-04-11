// Lightweight EPUB sanity checker.
//
// This is NOT a replacement for the official epubcheck (which is Java-only
// and ships as a 20MB JAR). Instead it catches the specific class of bugs
// we've actually hit in this codebase:
//
//   1. Chapter/page XHTML files that don't parse as well-formed XML
//      (e.g. non-self-closed void elements like <hr>/<img>).
//   2. Duplicate XML `id` attributes in content.opf (e.g. the epub-builder
//      0.8 dc:language / dc:creator collision).
//   3. Manifest hrefs that don't resolve to an actual file in the zip.
//   4. Spine idrefs that don't resolve to an actual manifest item id.
//
// Runs after every EPUB export in the happy path. Zero external deps
// beyond quick-xml which is already in Cargo.toml. Typical runtime is a
// few milliseconds per MB of book content.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashSet;
use std::io::{Cursor, Read};

#[derive(serde::Serialize, Clone)]
pub struct EpubIssue {
    pub level: String,   // "error" | "warning"
    pub code: String,    // short machine id, e.g. "xhtml-parse-error"
    pub file: String,    // which entry in the zip
    pub message: String, // human-readable detail
}

/// Validate an in-memory EPUB byte buffer. Returns a (possibly empty) list
/// of issues. An empty list means the file passes our sanity checks — it
/// does NOT guarantee epubcheck would pass (which is a much more thorough
/// tool). Use this to catch regressions of known bugs, not as a cert.
pub fn validate(epub_bytes: &[u8]) -> Vec<EpubIssue> {
    let mut issues: Vec<EpubIssue> = Vec::new();

    let reader = Cursor::new(epub_bytes);
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(e) => {
            issues.push(EpubIssue {
                level: "error".into(),
                code: "zip-open".into(),
                file: "(root)".into(),
                message: format!("Failed to open EPUB as zip: {}", e),
            });
            return issues;
        }
    };

    // Build a set of filenames in the zip for manifest href resolution.
    let mut zip_entries: HashSet<String> = HashSet::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            zip_entries.insert(f.name().to_string());
        }
    }

    // Walk every XHTML file and parse as XML. Any parse failure is an
    // error — it catches the <hr>/<img>/<br> self-closing bugs.
    for i in 0..archive.len() {
        let name = match archive.by_index(i) {
            Ok(f) => f.name().to_string(),
            Err(_) => continue,
        };
        if !name.ends_with(".xhtml") && !name.ends_with(".html") {
            continue;
        }
        let contents = match read_entry(&mut archive, &name) {
            Some(c) => c,
            None => continue,
        };
        if let Err(e) = parse_xml_well_formed(&contents) {
            issues.push(EpubIssue {
                level: "error".into(),
                code: "xhtml-parse-error".into(),
                file: name.clone(),
                message: format!("XHTML is not well-formed XML: {}", e),
            });
        }
    }

    // Parse content.opf. Check duplicate IDs, then validate manifest hrefs
    // and spine idrefs. Other OPF files like nav.xhtml / toc.ncx are already
    // covered by the XHTML parse loop above.
    if let Some(opf_bytes) = read_entry(&mut archive, "OEBPS/content.opf") {
        // 1. Parse error?
        if let Err(e) = parse_xml_well_formed(&opf_bytes) {
            issues.push(EpubIssue {
                level: "error".into(),
                code: "opf-parse-error".into(),
                file: "OEBPS/content.opf".into(),
                message: format!("content.opf is not well-formed XML: {}", e),
            });
        }

        // 2. Duplicate IDs?
        match collect_xml_ids(&opf_bytes) {
            Ok(ids) => {
                let mut seen = HashSet::new();
                let mut dupes = Vec::new();
                for id in &ids {
                    if !seen.insert(id.clone()) {
                        dupes.push(id.clone());
                    }
                }
                for d in dupes {
                    issues.push(EpubIssue {
                        level: "error".into(),
                        code: "opf-duplicate-id".into(),
                        file: "OEBPS/content.opf".into(),
                        message: format!(r#"Duplicate id="{}" in content.opf"#, d),
                    });
                }
            }
            Err(e) => {
                issues.push(EpubIssue {
                    level: "error".into(),
                    code: "opf-id-scan".into(),
                    file: "OEBPS/content.opf".into(),
                    message: format!("Failed to scan ids in OPF: {}", e),
                });
            }
        }

        // 3 & 4. Manifest hrefs + spine idrefs.
        match collect_manifest_and_spine(&opf_bytes) {
            Ok((manifest, spine)) => {
                // 3. Every manifest href should resolve to a zip entry.
                // OPF hrefs are relative to the OPF's directory (OEBPS/),
                // so prepend "OEBPS/" before checking the zip.
                for (id, href) in &manifest {
                    let full = format!("OEBPS/{}", href);
                    if !zip_entries.contains(&full) {
                        issues.push(EpubIssue {
                            level: "error".into(),
                            code: "manifest-missing-file".into(),
                            file: "OEBPS/content.opf".into(),
                            message: format!(
                                r#"Manifest item id="{}" points to "{}" which is not in the zip"#,
                                id, href
                            ),
                        });
                    }
                }

                // 4. Every spine idref should resolve to a manifest item.
                let manifest_ids: HashSet<&String> = manifest.iter().map(|(id, _)| id).collect();
                for idref in &spine {
                    if !manifest_ids.contains(idref) {
                        issues.push(EpubIssue {
                            level: "error".into(),
                            code: "spine-missing-manifest-item".into(),
                            file: "OEBPS/content.opf".into(),
                            message: format!(
                                r#"Spine references idref="{}" which is not in the manifest"#,
                                idref
                            ),
                        });
                    }
                }
            }
            Err(e) => {
                issues.push(EpubIssue {
                    level: "warning".into(),
                    code: "opf-structure-scan".into(),
                    file: "OEBPS/content.opf".into(),
                    message: format!("Could not scan manifest/spine: {}", e),
                });
            }
        }
    } else {
        issues.push(EpubIssue {
            level: "error".into(),
            code: "opf-missing".into(),
            file: "OEBPS/content.opf".into(),
            message: "content.opf not found in expected location".into(),
        });
    }

    issues
}

fn read_entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Parse an XML document and return the first error, or Ok if well-formed.
fn parse_xml_well_formed(bytes: &[u8]) -> Result<(), String> {
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(e) => return Err(format!("line {}: {}", reader.buffer_position(), e)),
        }
        buf.clear();
    }
}

/// Walk the XML and collect every `id` attribute value. Used to detect
/// duplicate IDs — a well-formed XML parser won't complain about duplicate
/// ids (they're a validity rule, not a well-formedness rule), so we have to
/// scan them ourselves.
fn collect_xml_ids(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut ids = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                for attr in e.attributes().with_checks(false).flatten() {
                    if attr.key.as_ref() == b"id" {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            ids.push(v.to_string());
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
        buf.clear();
    }
    Ok(ids)
}

/// Walk the OPF and collect `(id, href)` pairs from `<manifest>` and
/// idrefs from `<spine>`. Used to verify structural integrity.
fn collect_manifest_and_spine(
    bytes: &[u8],
) -> Result<(Vec<(String, String)>, Vec<String>), String> {
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut manifest: Vec<(String, String)> = Vec::new();
    let mut spine: Vec<String> = Vec::new();
    let mut in_manifest = false;
    let mut in_spine = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"manifest" {
                    in_manifest = true;
                } else if name.as_ref() == b"spine" {
                    in_spine = true;
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"manifest" {
                    in_manifest = false;
                } else if name.as_ref() == b"spine" {
                    in_spine = false;
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = e.name();
                let local = tag.as_ref();
                if in_manifest && local == b"item" {
                    let mut id = String::new();
                    let mut href = String::new();
                    for attr in e.attributes().with_checks(false).flatten() {
                        if attr.key.as_ref() == b"id" {
                            id = std::str::from_utf8(&attr.value).unwrap_or_default().to_string();
                        } else if attr.key.as_ref() == b"href" {
                            href = std::str::from_utf8(&attr.value).unwrap_or_default().to_string();
                        }
                    }
                    if !id.is_empty() && !href.is_empty() {
                        manifest.push((id, href));
                    }
                } else if in_spine && local == b"itemref" {
                    for attr in e.attributes().with_checks(false).flatten() {
                        if attr.key.as_ref() == b"idref" {
                            spine.push(std::str::from_utf8(&attr.value).unwrap_or_default().to_string());
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
        buf.clear();
    }

    Ok((manifest, spine))
}
