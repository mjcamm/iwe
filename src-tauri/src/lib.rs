mod db;
mod palettes;
mod scanner;
mod spellcheck;
mod synonyms;
mod wordlists;

use db::AppState;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

// ---- Project commands ----

#[tauri::command]
fn open_project(state: tauri::State<'_, AppState>, filepath: String) -> Result<(), String> {
    let conn = Connection::open(&filepath).map_err(|e| e.to_string())?;
    db::init_schema(&conn).map_err(|e| e.to_string())?;
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    Ok(())
}

// ---- Chapter commands ----

#[tauri::command]
fn get_chapters(state: tauri::State<'_, AppState>) -> Result<Vec<db::Chapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_chapters(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<db::Chapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_chapter(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_chapter(state: tauri::State<'_, AppState>, title: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_chapter(conn, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_chapter_content(state: tauri::State<'_, AppState>, id: i64, content: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_chapter_content(conn, id, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_chapter(state: tauri::State<'_, AppState>, id: i64, title: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::rename_chapter(conn, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_chapter(conn, id).map_err(|e| e.to_string())
}

// ---- Entity commands ----

#[tauri::command]
fn get_entities(state: tauri::State<'_, AppState>) -> Result<Vec<db::Entity>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_entities(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_entity(state: tauri::State<'_, AppState>, name: String, entity_type: String, description: String, color: Option<String>) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let id = db::create_entity(conn, &name, &entity_type, &description, &color.unwrap_or_default()).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(id)
}

#[tauri::command]
fn update_entity(state: tauri::State<'_, AppState>, id: i64, name: String, entity_type: String, description: String, color: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_entity(conn, id, &name, &entity_type, &description, &color).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn delete_entity(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity(conn, id).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn set_entity_visible(state: tauri::State<'_, AppState>, id: i64, visible: bool) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::set_entity_visible(conn, id, visible).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_alias(state: tauri::State<'_, AppState>, entity_id: i64, alias: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_alias(conn, entity_id, &alias).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn remove_alias(state: tauri::State<'_, AppState>, entity_id: i64, alias: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_alias(conn, entity_id, &alias).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

// ---- Entity notes ----

#[tauri::command]
fn get_entity_notes(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::EntityNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_notes(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_entity_note(state: tauri::State<'_, AppState>, entity_id: i64, chapter_id: Option<i64>, text: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_entity_note(conn, entity_id, chapter_id, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity_note(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_entity_notes(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_entity_notes(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_entity_free_notes(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::EntityFreeNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_free_notes(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_entity_free_note(state: tauri::State<'_, AppState>, entity_id: i64, text: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_entity_free_note(conn, entity_id, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_entity_free_note(state: tauri::State<'_, AppState>, id: i64, text: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_entity_free_note(conn, id, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity_free_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity_free_note(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_entity_free_notes(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_entity_free_notes(conn, &ids).map_err(|e| e.to_string())
}

// ---- Writing stats ----

#[tauri::command]
fn log_writing_activity(state: tauri::State<'_, AppState>, chapter_id: Option<i64>, chapter_words: i64, manuscript_words: i64, words_delta: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::log_writing_activity(conn, chapter_id, chapter_words, manuscript_words, words_delta).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_daily_stats(state: tauri::State<'_, AppState>, days: i64) -> Result<Vec<db::DailyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_daily_stats(conn, days).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_daily_stats(state: tauri::State<'_, AppState>) -> Result<Vec<db::DailyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_all_daily_stats(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_writing_settings(state: tauri::State<'_, AppState>) -> Result<db::WritingSettings, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_writing_settings(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_writing_settings(state: tauri::State<'_, AppState>, daily_goal: i64, session_gap_minutes: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_writing_settings(conn, daily_goal, session_gap_minutes).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_writing_activity(state: tauri::State<'_, AppState>, date: String) -> Result<Vec<db::WritingActivity>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_writing_activity(conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_manuscript_word_history(state: tauri::State<'_, AppState>) -> Result<Vec<(String, i64)>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_manuscript_word_history(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_hourly_breakdown(state: tauri::State<'_, AppState>, date: String) -> Result<Vec<db::HourlyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_hourly_breakdown(conn, &date).map_err(|e| e.to_string())
}

// ---- Navigation history ----

#[tauri::command]
fn get_nav_history(state: tauri::State<'_, AppState>) -> Result<Vec<db::NavEntry>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_nav_history(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn push_nav_entry(state: tauri::State<'_, AppState>, chapter_id: i64, scroll_top: f64, cursor_pos: i64) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::push_nav_entry(conn, chapter_id, scroll_top, cursor_pos).map_err(|e| e.to_string())
}

#[tauri::command]
fn truncate_nav_after(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::truncate_nav_after(conn, id).map_err(|e| e.to_string())
}

// ---- PDF export ----

fn strip_html_to_text(html: &str) -> String {
    html.replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</h1>", "\n\n")
        .replace("</h2>", "\n\n")
        .replace("</h3>", "\n\n")
        .replace("<hr>", "\n\n* * *\n\n")
        .replace("<hr/>", "\n\n* * *\n\n")
        .replace("</li>", "\n")
        .replace("</blockquote>", "\n")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split('<')
        .map(|s| s.split_once('>').map(|(_, rest)| rest).unwrap_or(s))
        .collect::<Vec<_>>()
        .join("")
}

#[tauri::command]
fn export_pdf(state: tauri::State<'_, AppState>, path: String, title: String, format: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let is_book = format == "book";

    // Embedded Liberation Serif fonts — bundled in the binary, full Unicode support
    let regular = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Regular.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let bold = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Bold.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let italic = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Italic.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let bold_italic = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;

    let font_family = genpdf::fonts::FontFamily {
        regular,
        bold,
        italic,
        bold_italic,
    };
    let mut doc = genpdf::Document::new(font_family);

    // Page setup
    let page_size = if is_book {
        genpdf::Size::new(127, 203) // 5x8 inches in mm
    } else {
        genpdf::Size::new(210, 297) // A4
    };

    struct NumberedPageDecorator {
        margins: genpdf::Margins,
        page: usize,
    }

    impl genpdf::PageDecorator for NumberedPageDecorator {
        fn decorate_page<'a>(
            &mut self,
            context: &genpdf::Context,
            mut area: genpdf::render::Area<'a>,
            style: genpdf::style::Style,
        ) -> Result<genpdf::render::Area<'a>, genpdf::error::Error> {
            self.page += 1;
            area.add_margins(self.margins);

            // Render page number at bottom (skip page 1 = title page)
            if self.page > 1 {
                // Shrink content area to leave room for footer
                let footer_height = genpdf::Mm::from(8);
                let content_height = area.size().height - footer_height;

                // Print footer at the bottom
                let mut footer_area = area.clone();
                footer_area.add_offset(genpdf::Position::new(0, content_height));
                let page_str = format!("{}", self.page - 1);
                let mut para = genpdf::elements::Paragraph::new(&page_str);
                para.set_alignment(genpdf::Alignment::Center);
                use genpdf::Element;
                para.render(context, footer_area, style.with_font_size(9))?;

                // Reduce content area so text doesn't overlap footer
                area.set_height(content_height);
            }

            Ok(area)
        }
    }

    let margins = if is_book {
        genpdf::Margins::trbl(15, 12, 20, 12)
    } else {
        genpdf::Margins::trbl(25, 25, 30, 25)
    };

    doc.set_paper_size(page_size);
    doc.set_page_decorator(NumberedPageDecorator { margins, page: 0 });
    doc.set_font_size(if is_book { 10 } else { 12 });
    doc.set_line_spacing(1.5);
    doc.set_title(&title);

    // Title page
    doc.push(genpdf::elements::Break::new(5.0));
    let mut title_para = genpdf::elements::Paragraph::new("");
    title_para.push(genpdf::style::StyledString::new(
        &title,
        genpdf::style::Style::new().bold().with_font_size(if is_book { 18 } else { 24 }),
    ));
    title_para.set_alignment(genpdf::Alignment::Center);
    doc.push(title_para);
    doc.push(genpdf::elements::Break::new(20.0));
    doc.push(genpdf::elements::PageBreak::new());

    // Chapters
    for chapter in &chapters {
        // Chapter title
        let mut ch_title = genpdf::elements::Paragraph::new("");
        ch_title.push(genpdf::style::StyledString::new(
            &chapter.title,
            genpdf::style::Style::new().bold().with_font_size(if is_book { 14 } else { 16 }),
        ));
        ch_title.set_alignment(genpdf::Alignment::Center);
        doc.push(ch_title);
        doc.push(genpdf::elements::Break::new(1.0));

        // Chapter content
        let plain = strip_html_to_text(&chapter.content);
        for paragraph in plain.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() { continue; }
            if trimmed == "* * *" {
                let mut sep = genpdf::elements::Paragraph::new("* * *");
                sep.set_alignment(genpdf::Alignment::Center);
                doc.push(genpdf::elements::Break::new(0.5));
                doc.push(sep);
                doc.push(genpdf::elements::Break::new(0.5));
            } else {
                doc.push(genpdf::elements::Paragraph::new(trimmed));
            }
        }

        doc.push(genpdf::elements::PageBreak::new());
    }

    doc.render_to_file(&path).map_err(|e| e.to_string())?;

    Ok(())
}

// ---- Ignored words ----

#[tauri::command]
fn add_ignored_word(state: tauri::State<'_, AppState>, word: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_ignored_word(conn, &word).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_ignored_word(state: tauri::State<'_, AppState>, word: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_ignored_word(conn, &word).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize spell checker and synonym state at startup
    let spell_state = spellcheck::init_spellcheck();
    let synonym_state = synonyms::init_synonyms();

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(None),
        })
        .manage(spell_state)
        .manage(synonym_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            open_project,
            get_chapters,
            get_chapter,
            add_chapter,
            update_chapter_content,
            rename_chapter,
            delete_chapter,
            get_entities,
            create_entity,
            update_entity,
            delete_entity,
            set_entity_visible,
            add_alias,
            get_entity_notes,
            add_entity_note,
            delete_entity_note,
            reorder_entity_notes,
            get_entity_free_notes,
            add_entity_free_note,
            update_entity_free_note,
            delete_entity_free_note,
            reorder_entity_free_notes,
            log_writing_activity,
            get_daily_stats,
            get_all_daily_stats,
            get_writing_settings,
            update_writing_settings,
            get_writing_activity,
            get_hourly_breakdown,
            get_manuscript_word_history,
            get_nav_history,
            push_nav_entry,
            truncate_nav_after,
            export_pdf,
            add_ignored_word,
            remove_ignored_word,
            remove_alias,
            scanner::scan_text,
            scanner::scan_all_chapters,
            scanner::detect_entities,
            scanner::check_word,
            scanner::debug_search_terms,
            scanner::find_references,
            scanner::relationship_search,
            scanner::text_search,
            scanner::dialogue_search,
            scanner::word_frequency,
            scanner::find_similar_phrases,
            scanner::generate_heatmap,
            scanner::chapter_analysis,
            spellcheck::check_spelling,
            spellcheck::get_spell_suggestions,
            spellcheck::debug_spell_check,
            spellcheck::add_to_dictionary,
            spellcheck::remove_from_dictionary,
            spellcheck::get_custom_words,
            spellcheck::set_spell_language,
            spellcheck::get_spell_language,
            synonyms::get_synonyms,
            palettes::get_palettes,
            palettes::create_palette,
            palettes::update_palette,
            palettes::delete_palette,
            palettes::toggle_palette,
            palettes::copy_palette,
            palettes::get_word_groups,
            palettes::get_word_group,
            palettes::create_word_group,
            palettes::update_word_group,
            palettes::delete_word_group,
            palettes::get_all_section_names,
            palettes::add_section,
            palettes::add_all_sections,
            palettes::rename_section,
            palettes::delete_section,
            palettes::add_word_entry,
            palettes::add_word_entries,
            palettes::remove_word_entry,
            palettes::search_word_groups,
            palettes::get_active_groups,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Initialize palettes DB in app data directory
            let app_data = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data).ok();
            let palette_path = app_data.join("palettes.db");
            let palette_conn = palettes::init_palette_db(
                palette_path.to_str().expect("Invalid palette path")
            ).expect("Failed to init palettes DB");
            app.manage(palettes::PaletteState {
                db: Mutex::new(palette_conn),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
