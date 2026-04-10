// When the "semantic" feature is enabled, the real implementation lives in
// semantic_impl.rs.  When disabled, lightweight stubs are provided so that
// lib.rs compiles without pulling in ort / tokenizers.

#[cfg(feature = "semantic")]
mod real {
    use crate::db::{self, AppState};
    use crate::text_utils;
    use crate::ydoc;
    use ort::session::Session;
    use ort::value::Value;
    use rusqlite::{params, Connection};
    use serde::Serialize;
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tauri::{Emitter, Manager};
    use tokenizers::Tokenizer;

    // ---- State ----

    pub struct SemanticState {
        session: Mutex<Option<Arc<Mutex<Session>>>>,
        tokenizer: Mutex<Option<Arc<Tokenizer>>>,
        dirty_chapters: Arc<Mutex<HashSet<i64>>>,
        indexing_active: Arc<AtomicBool>,
    }

    impl SemanticState {
        pub fn new() -> Self {
            Self {
                session: Mutex::new(None),
                tokenizer: Mutex::new(None),
                dirty_chapters: Arc::new(Mutex::new(HashSet::new())),
                indexing_active: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    // ---- Resolve model directory ----

    fn resolve_model_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
        let via_resource = app_handle
            .path()
            .resolve("resources/models", tauri::path::BaseDirectory::Resource);
        if let Ok(p) = via_resource {
            if p.join("model.onnx").exists() {
                return Ok(p);
            }
        }

        let dev_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("models");
        if dev_path.join("model.onnx").exists() {
            return Ok(dev_path);
        }

        Err("Could not find model files. Ensure resources/models/model.onnx exists.".to_string())
    }

    // ---- Model loading ----

    fn ensure_loaded(
        state: &SemanticState,
        model_dir: &Path,
    ) -> Result<(Arc<Mutex<Session>>, Arc<Tokenizer>), String> {
        let mut session_guard = state.session.lock().map_err(|e| e.to_string())?;
        let mut tok_guard = state.tokenizer.lock().map_err(|e| e.to_string())?;

        if session_guard.is_none() {
            let model_path = model_dir.join("model.onnx");
            let session = Session::builder()
                .map_err(|e| format!("ONNX session builder: {e}"))?
                .with_intra_threads(2)
                .map_err(|e| format!("ONNX threads: {e}"))?
                .commit_from_file(&model_path)
                .map_err(|e| format!("Failed to load ONNX model: {e}"))?;
            *session_guard = Some(Arc::new(Mutex::new(session)));
        }

        if tok_guard.is_none() {
            let tok_path = model_dir.join("tokenizer.json");
            let mut tokenizer = Tokenizer::from_file(&tok_path)
                .map_err(|e| format!("Failed to load tokenizer: {e}"))?;
            let trunc = tokenizers::TruncationParams {
                max_length: 512,
                ..Default::default()
            };
            let _ = tokenizer.with_truncation(Some(trunc));
            let _ = tokenizer.with_padding(None);
            *tok_guard = Some(Arc::new(tokenizer));
        }

        Ok((
            session_guard.as_ref().unwrap().clone(),
            tok_guard.as_ref().unwrap().clone(),
        ))
    }

    // ---- Embedding generation ----

    fn generate_embedding(
        session_mtx: &Mutex<Session>,
        tokenizer: &Tokenizer,
        text: &str,
    ) -> Result<Vec<f32>, String> {
        let mut session = session_mtx.lock().map_err(|e| e.to_string())?;
        let encoding = tokenizer.encode(text, true).map_err(|e| e.to_string())?;
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();
        let type_ids = encoding.get_type_ids();
        let seq_len = ids.len();

        let input_ids: Vec<i64> = ids.iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = mask.iter().map(|&m| m as i64).collect();

        let shape = [1usize, seq_len];
        let input_ids_val =
            Value::from_array((shape, input_ids.into_boxed_slice()))
                .map_err(|e| e.to_string())?;
        let attention_mask_val =
            Value::from_array((shape, attention_mask.clone().into_boxed_slice()))
                .map_err(|e| e.to_string())?;

        let num_inputs = session.inputs().len();
        let outputs = if num_inputs <= 2 {
            session
                .run(ort::inputs![input_ids_val, attention_mask_val])
                .map_err(|e| format!("ONNX inference failed: {e}"))?
        } else {
            let token_type_ids: Vec<i64> = type_ids.iter().map(|&t| t as i64).collect();
            let token_type_ids_val =
                Value::from_array((shape, token_type_ids.into_boxed_slice()))
                    .map_err(|e| e.to_string())?;
            session
                .run(ort::inputs![input_ids_val, attention_mask_val, token_type_ids_val])
                .map_err(|e| format!("ONNX inference failed: {e}"))?
        };

        let output = &outputs[0];
        let (out_shape, out_data) = output.try_extract_tensor::<f32>().map_err(|e| format!("Extract failed: {e}"))?;

        let hidden_size = out_shape[2] as usize;
        let mut pooled = vec![0.0f32; hidden_size];
        let mut mask_sum = 0.0f32;

        for t in 0..seq_len {
            let m = attention_mask[t] as f32;
            mask_sum += m;
            for h in 0..hidden_size {
                pooled[h] += out_data[t * hidden_size + h] * m;
            }
        }

        if mask_sum > 0.0 {
            for h in 0..hidden_size {
                pooled[h] /= mask_sum;
            }
        }

        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in pooled.iter_mut() {
                *v /= norm;
            }
        }

        Ok(pooled)
    }

    // ---- Text segmentation ----

    #[derive(Clone)]
    struct TextSegment {
        text: String,
        char_start: usize,
        char_end: usize,
    }

    fn extract_paragraphs(text_with_breaks: &str) -> Vec<TextSegment> {
        let mut segments = Vec::new();
        let mut offset = 0;

        for part in text_with_breaks.split("\n\n") {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                let start = text_with_breaks[offset..]
                    .find(trimmed)
                    .map(|i| offset + i)
                    .unwrap_or(offset);
                segments.push(TextSegment {
                    text: trimmed.to_string(),
                    char_start: start,
                    char_end: start + trimmed.len(),
                });
            }
            offset += part.len() + 2;
        }

        segments
    }

    // ---- Embedding serialization ----

    fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
        let mut blob = Vec::with_capacity(embedding.len() * 4);
        for &val in embedding {
            blob.extend_from_slice(&val.to_le_bytes());
        }
        blob
    }

    fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    // ---- DB helpers ----

    fn clear_chapter_embeddings(conn: &Connection, chapter_id: i64) -> rusqlite::Result<()> {
        conn.execute("DELETE FROM semantic_embeddings WHERE chapter_id = ?1", params![chapter_id])?;
        Ok(())
    }

    fn insert_embedding(
        conn: &Connection,
        chapter_id: i64,
        granularity: &str,
        segment_text: &str,
        char_start: usize,
        char_end: usize,
        embedding: &[u8],
    ) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO semantic_embeddings (chapter_id, granularity, segment_text, char_start, char_end, embedding) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chapter_id, granularity, segment_text, char_start as i64, char_end as i64, embedding],
        )?;
        Ok(())
    }

    struct EmbeddingRow {
        chapter_id: i64,
        segment_text: String,
        char_start: usize,
        embedding: Vec<f32>,
    }

    fn get_all_embeddings(conn: &Connection, granularity: &str) -> rusqlite::Result<Vec<EmbeddingRow>> {
        let mut stmt = conn.prepare(
            "SELECT e.chapter_id, e.segment_text, e.char_start, e.embedding FROM semantic_embeddings e INNER JOIN chapters c ON e.chapter_id = c.id WHERE e.granularity = ?1 AND COALESCE(c.deleted, 0) = 0 ORDER BY c.sort_order ASC, e.char_start ASC",
        )?;
        let rows = stmt.query_map(params![granularity], |row| {
            let blob: Vec<u8> = row.get(3)?;
            Ok(EmbeddingRow {
                chapter_id: row.get(0)?,
                segment_text: row.get(1)?,
                char_start: row.get::<_, i64>(2)? as usize,
                embedding: blob_to_embedding(&blob),
            })
        })?;
        rows.collect()
    }

    fn update_index_status(conn: &Connection, chapter_id: i64, content_hash: &str) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO semantic_index_status (chapter_id, content_hash, indexed_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            params![chapter_id, content_hash],
        )?;
        Ok(())
    }

    fn simple_hash(text: &str) -> String {
        let len = text.len();
        let first: String = text.chars().take(50).collect();
        let last: String = text.chars().rev().take(50).collect::<String>().chars().rev().collect();
        format!("{len}:{first}:{last}")
    }

    // ---- Index a single chapter ----

    fn index_chapter(
        conn: &Connection,
        session: &Mutex<Session>,
        tokenizer: &Tokenizer,
        chapter_id: i64,
        content: &[u8],
    ) -> Result<(), String> {
        let doc = ydoc::load_doc(content)?;
        let plain_text = ydoc::extract_plain_text(&doc);
        let text_with_breaks = ydoc::extract_text_with_breaks(&doc);

        if plain_text.trim().is_empty() {
            clear_chapter_embeddings(conn, chapter_id).map_err(|e| e.to_string())?;
            update_index_status(conn, chapter_id, "empty").map_err(|e| e.to_string())?;
            return Ok(());
        }

        let hash = simple_hash(&plain_text);

        clear_chapter_embeddings(conn, chapter_id).map_err(|e| e.to_string())?;

        let throttle = std::time::Duration::from_millis(5);

        let sentences = text_utils::extract_sentences(&plain_text);
        for sent in &sentences {
            if sent.word_count < 3 {
                continue;
            }
            let emb = generate_embedding(session, tokenizer, &sent.text)?;
            let blob = embedding_to_blob(&emb);
            insert_embedding(conn, chapter_id, "sentence", &sent.text, sent.char_start, sent.char_end, &blob)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(throttle);
        }

        let paragraphs = extract_paragraphs(&text_with_breaks);
        for para in &paragraphs {
            if para.text.split_whitespace().count() < 3 {
                continue;
            }
            let emb = generate_embedding(session, tokenizer, &para.text)?;
            let blob = embedding_to_blob(&emb);
            insert_embedding(conn, chapter_id, "paragraph", &para.text, para.char_start, para.char_end, &blob)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(throttle);
        }

        update_index_status(conn, chapter_id, &hash).map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---- Tauri commands ----

    #[derive(Serialize)]
    pub struct SemanticSearchResult {
        pub chapter_id: i64,
        pub chapter_title: String,
        pub segment_text: String,
        pub char_start: usize,
        pub score: f32,
    }

    #[derive(Serialize)]
    pub struct SemanticIndexInfo {
        pub total_chapters: usize,
        pub indexed_chapters: usize,
        pub total_embeddings: usize,
        pub pending: usize,
    }

    #[tauri::command]
    pub fn mark_chapter_dirty(
        sem_state: tauri::State<'_, SemanticState>,
        chapter_id: i64,
    ) -> Result<(), String> {
        let mut dirty = sem_state.dirty_chapters.lock().map_err(|e| e.to_string())?;
        dirty.insert(chapter_id);
        Ok(())
    }

    #[tauri::command]
    pub fn run_semantic_indexing(
        app_state: tauri::State<'_, AppState>,
        sem_state: tauri::State<'_, SemanticState>,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        if sem_state.indexing_active.swap(true, Ordering::SeqCst) {
            log::info!("[semantic] indexing already active, skipping");
            return Ok(());
        }

        let dirty: Vec<i64> = {
            let mut guard = sem_state.dirty_chapters.lock().map_err(|e| e.to_string())?;
            guard.drain().collect()
        };

        if dirty.is_empty() {
            log::info!("[semantic] no dirty chapters, nothing to index");
            sem_state.indexing_active.store(false, Ordering::SeqCst);
            return Ok(());
        }

        let db_path = {
            let guard = app_state.db_path.lock().map_err(|e| e.to_string())?;
            guard.clone().ok_or("No project open")?
        };

        let model_dir = resolve_model_dir(&app_handle)?;
        let (session, tokenizer) = ensure_loaded(&sem_state, &model_dir)?;

        let indexing_flag = sem_state.indexing_active.clone();

        std::thread::spawn(move || {
            let _ = app_handle.emit("semantic-index-started", ());
            log::info!("[semantic] background thread started for {} chapters", dirty.len());

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let conn = Connection::open(&db_path).map_err(|e| format!("DB open: {e}"))?;

                for (i, chapter_id) in dirty.iter().enumerate() {
                    let chapter = match db::get_chapter(&conn, *chapter_id) {
                        Ok(Some(ch)) => ch,
                        Ok(None) => continue,
                        Err(e) => { log::warn!("[semantic] read chapter {} failed: {}", chapter_id, e); continue; }
                    };
                    if chapter.deleted { continue; }

                    log::info!("[semantic] indexing chapter {} '{}'", chapter.id, chapter.title);
                    let _ = app_handle.emit("semantic-index-progress", serde_json::json!({
                        "done": i + 1,
                        "total": dirty.len(),
                        "chapter": chapter.title,
                    }));

                    if let Err(e) = index_chapter(&conn, &session, &tokenizer, chapter.id, &chapter.content) {
                        log::warn!("[semantic] index chapter {} failed: {}", chapter.id, e);
                    } else {
                        log::info!("[semantic] indexed chapter {} successfully", chapter.id);
                    }
                }

                Ok::<(), String>(())
            }));

            match result {
                Ok(Ok(())) => log::info!("[semantic] indexing complete"),
                Ok(Err(e)) => log::error!("[semantic] indexing error: {}", e),
                Err(panic) => log::error!("[semantic] indexing panicked: {:?}", panic),
            }

            indexing_flag.store(false, Ordering::SeqCst);
            let _ = app_handle.emit("semantic-index-updated", ());
        });

        Ok(())
    }

    #[tauri::command]
    pub fn rebuild_semantic_index(
        app_state: tauri::State<'_, AppState>,
        sem_state: tauri::State<'_, SemanticState>,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        {
            let guard = app_state.db.lock().map_err(|e| e.to_string())?;
            let conn = guard.as_ref().ok_or("No project open")?;
            conn.execute("DELETE FROM semantic_embeddings", []).map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM semantic_index_status", []).map_err(|e| e.to_string())?;
            log::info!("[semantic] flushed all embeddings and index status");
        }

        let chapters = {
            let guard = app_state.db.lock().map_err(|e| e.to_string())?;
            let conn = guard.as_ref().ok_or("No project open")?;
            db::list_chapters(conn).map_err(|e| e.to_string())?
        };

        {
            let mut dirty = sem_state.dirty_chapters.lock().map_err(|e| e.to_string())?;
            for ch in &chapters {
                dirty.insert(ch.id);
            }
        }

        sem_state.indexing_active.store(false, Ordering::SeqCst);

        run_semantic_indexing(app_state, sem_state, app_handle)
    }

    #[tauri::command]
    pub fn semantic_search(
        app_state: tauri::State<'_, AppState>,
        sem_state: tauri::State<'_, SemanticState>,
        app_handle: tauri::AppHandle,
        query: String,
        granularity: String,
        top_n: Option<usize>,
    ) -> Result<Vec<SemanticSearchResult>, String> {
        let model_dir = resolve_model_dir(&app_handle)?;
        let (session, tokenizer) = ensure_loaded(&sem_state, &model_dir)?;

        let query_emb = generate_embedding(&session, &tokenizer, &query)?;

        let guard = app_state.db.lock().map_err(|e| e.to_string())?;
        let conn = guard.as_ref().ok_or("No project open")?;
        let rows = get_all_embeddings(conn, &granularity).map_err(|e| e.to_string())?;
        let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
        drop(guard);

        let chapter_titles: std::collections::HashMap<i64, String> =
            chapters.into_iter().map(|ch| (ch.id, ch.title)).collect();

        let mut results: Vec<SemanticSearchResult> = rows
            .iter()
            .map(|row| {
                let score = dot_product(&query_emb, &row.embedding);
                SemanticSearchResult {
                    chapter_id: row.chapter_id,
                    chapter_title: chapter_titles.get(&row.chapter_id).cloned().unwrap_or_default(),
                    segment_text: row.segment_text.clone(),
                    char_start: row.char_start,
                    score,
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_n.unwrap_or(20));

        Ok(results)
    }

    #[tauri::command]
    pub fn get_semantic_index_status(
        app_state: tauri::State<'_, AppState>,
        sem_state: tauri::State<'_, SemanticState>,
    ) -> Result<SemanticIndexInfo, String> {
        let guard = app_state.db.lock().map_err(|e| e.to_string())?;
        let conn = guard.as_ref().ok_or("No project open")?;

        let total_chapters: i64 = conn
            .query_row("SELECT COUNT(*) FROM chapters WHERE COALESCE(deleted, 0) = 0", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let indexed_chapters: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM semantic_index_status s INNER JOIN chapters c ON s.chapter_id = c.id WHERE COALESCE(c.deleted, 0) = 0",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let total_embeddings: i64 = conn
            .query_row("SELECT COUNT(*) FROM semantic_embeddings", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let pending = {
            let dirty = sem_state.dirty_chapters.lock().map_err(|e| e.to_string())?;
            dirty.len()
        };

        Ok(SemanticIndexInfo {
            total_chapters: total_chapters as usize,
            indexed_chapters: indexed_chapters as usize,
            total_embeddings: total_embeddings as usize,
            pending,
        })
    }
}

// ---- Stubs when "semantic" feature is disabled ----

#[cfg(not(feature = "semantic"))]
mod real {
    use serde::Serialize;

    pub struct SemanticState;
    impl SemanticState {
        pub fn new() -> Self { SemanticState }
    }

    #[derive(Serialize)]
    pub struct SemanticSearchResult {
        pub chapter_id: i64,
        pub chapter_title: String,
        pub segment_text: String,
        pub char_start: usize,
        pub score: f32,
    }

    #[derive(Serialize)]
    pub struct SemanticIndexInfo {
        pub total_chapters: usize,
        pub indexed_chapters: usize,
        pub total_embeddings: usize,
        pub pending: usize,
    }

    #[tauri::command]
    pub fn mark_chapter_dirty(_chapter_id: i64) -> Result<(), String> {
        Err("Semantic features not enabled (build with --features semantic)".into())
    }

    #[tauri::command]
    pub fn run_semantic_indexing() -> Result<(), String> {
        Err("Semantic features not enabled (build with --features semantic)".into())
    }

    #[tauri::command]
    pub fn rebuild_semantic_index() -> Result<(), String> {
        Err("Semantic features not enabled (build with --features semantic)".into())
    }

    #[tauri::command]
    pub fn semantic_search() -> Result<Vec<SemanticSearchResult>, String> {
        Err("Semantic features not enabled (build with --features semantic)".into())
    }

    #[tauri::command]
    pub fn get_semantic_index_status() -> Result<SemanticIndexInfo, String> {
        Err("Semantic features not enabled (build with --features semantic)".into())
    }
}

// Re-export everything from the selected implementation
pub use real::*;
