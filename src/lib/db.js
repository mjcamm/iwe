import { invoke } from '@tauri-apps/api/core';
import { readDir, readTextFile, writeTextFile, exists, mkdir, remove, BaseDirectory } from '@tauri-apps/plugin-fs';
import { appDataDir } from '@tauri-apps/api/path';

const SETTINGS_FILE = 'settings.json';

// --- Settings (stored in app data dir as JSON) ---

async function ensureAppDataDir() {
  const dir = await appDataDir();
  if (!(await exists(dir))) {
    await mkdir(dir, { recursive: true });
  }
}

export async function getSettings() {
  await ensureAppDataDir();
  try {
    const text = await readTextFile(SETTINGS_FILE, { baseDir: BaseDirectory.AppData });
    return JSON.parse(text);
  } catch {
    return {};
  }
}

export async function saveSettings(settings) {
  await ensureAppDataDir();
  await writeTextFile(SETTINGS_FILE, JSON.stringify(settings, null, 2), { baseDir: BaseDirectory.AppData });
}

export async function getProjectsDir() {
  const settings = await getSettings();
  return settings.projectsDir || null;
}

export async function setProjectsDir(dir) {
  const settings = await getSettings();
  settings.projectsDir = dir;
  await saveSettings(settings);
}

// --- Project listing (scan folder for .iwe files) ---

export async function listProjects() {
  const dir = await getProjectsDir();
  if (!dir) return [];

  try {
    const entries = await readDir(dir);
    const projects = [];

    for (const entry of entries) {
      if (entry.name && entry.name.endsWith('.iwe')) {
        projects.push({
          filename: entry.name,
          filepath: `${dir}/${entry.name}`,
          title: entry.name.replace('.iwe', '')
        });
      }
    }

    return projects.sort((a, b) => a.title.localeCompare(b.title));
  } catch {
    return [];
  }
}

// --- Image blobs (shared BLOB storage, per-project) ---

// Upload raw bytes as an image. Returns the image_blobs.id (deduped by SHA-256
// hash, so re-uploading identical bytes returns the same id).
export async function uploadImage(data, mime) {
  const bytes = data instanceof Uint8Array ? Array.from(data) : data;
  return invoke('upload_image', { bytes, mime });
}

// Returns { data: number[], mime: string } | null
export async function getImageBlob(id) {
  if (id == null) return null;
  return invoke('get_image_blob', { id });
}

export async function deleteImageBlob(id) {
  return invoke('delete_image_blob', { id });
}

export async function listOrphanImages() {
  return invoke('list_orphan_images');
}

export async function cleanupOrphanImages() {
  return invoke('cleanup_orphan_images');
}

// Build the `<img src>` URL that hits the iwe-image:// custom scheme handler.
// Tauri v2 normalizes custom URI schemes to http://{scheme}.localhost/... for
// WebView2 fetches, matching the existing iwe:// → http://iwe.localhost pattern.
export function imageSrcFor(id) {
  return id == null ? null : `http://iwe-image.localhost/${id}`;
}

// Read a file (File/Blob) and upload it to image_blobs. Returns the new id.
export async function uploadImageFile(file) {
  const buf = await file.arrayBuffer();
  const bytes = new Uint8Array(buf);
  return uploadImage(bytes, file.type || 'application/octet-stream');
}

// --- Book cover ---
//
// The cover is just another image_blobs row referenced by the
// `cover_image_id` key in `project_settings`. These wrappers keep the old
// API shape so callers don't care about the backing store.

// Returns { data: number[], mime_type: string } | null
export async function getBookCover() {
  return invoke('get_book_cover');
}

// data: Uint8Array or number[]; mimeType: string like "image/jpeg".
// Returns the new image_blobs id.
export async function setBookCover(data, mimeType) {
  const bytes = data instanceof Uint8Array ? Array.from(data) : data;
  return invoke('set_book_cover', { data: bytes, mimeType });
}

export async function clearBookCover() {
  return invoke('clear_book_cover');
}

// Peek the cover of any .iwe file by path, without opening it as the current
// project. Used by the home page to show cover thumbnails. Returns null if
// the project has no cover.
export async function getProjectCoverByPath(filepath) {
  return invoke('get_project_cover_by_path', { filepath });
}

// Helper: convert a BookCoverData response into an object URL the browser
// can use directly as an <img src>. Caller is responsible for calling
// URL.revokeObjectURL() when done, or accepting the GC cleanup.
export function coverToObjectUrl(cover) {
  if (!cover || !cover.data || cover.data.length === 0) return null;
  const bytes = new Uint8Array(cover.data);
  const blob = new Blob([bytes], { type: cover.mime_type || 'image/jpeg' });
  return URL.createObjectURL(blob);
}

// --- Project creation ---

export async function createProject(title) {
  const dir = await getProjectsDir();
  if (!dir) throw new Error('No projects directory set');

  const filename = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '') + '.iwe';
  const filepath = `${dir}/${filename}`;

  // Open project initializes schema
  await invoke('open_project', { filepath });

  // Create first chapter
  await invoke('add_chapter', { title: 'Chapter 1' });

  return filename;
}

export async function deleteProject(filepath) {
  // Make sure we don't hold the SQLite file open
  try { await invoke('close_project'); } catch {}
  await remove(filepath);
}

// --- Project operations (all via Rust commands) ---

export async function openProject(filepath) {
  await invoke('open_project', { filepath });
}

export async function getChapters() {
  return invoke('get_chapters');
}

export async function getChapter(id) {
  return invoke('get_chapter', { id });
}

export async function addChapter(title) {
  return invoke('add_chapter', { title });
}

export async function updateChapterContent(id, content) {
  // content is a Uint8Array (Y.Doc state) — convert to number array for Tauri IPC
  return invoke('update_chapter_content', { id, content: Array.from(content) });
}

export async function renameChapter(id, title) {
  return invoke('rename_chapter', { id, title });
}

export async function deleteChapter(id) {
  return invoke('delete_chapter', { id });
}

export async function reorderChapters(ids) {
  return invoke('reorder_chapters', { ids });
}

export async function getDeletedChapters() {
  return invoke('get_deleted_chapters');
}

export async function restoreChapter(id) {
  return invoke('restore_chapter', { id });
}

// --- Entity operations ---

export async function getEntities() {
  return invoke('get_entities');
}

export async function createEntity(name, entityType, description = '', color = '') {
  return invoke('create_entity', { name, entityType, description, color: color || null });
}

export async function updateEntity(id, name, entityType, description, color) {
  return invoke('update_entity', { id, name, entityType, description, color });
}

export async function setEntityVisible(id, visible) {
  return invoke('set_entity_visible', { id, visible });
}

export async function deleteEntity(id) {
  return invoke('delete_entity', { id });
}

export async function addAlias(entityId, alias) {
  return invoke('add_alias', { entityId, alias });
}

export async function removeAlias(entityId, alias) {
  return invoke('remove_alias', { entityId, alias });
}

// --- Scanner ---

export async function scanText(text) {
  return invoke('scan_text', { text });
}

export async function scanAllChapters() {
  return invoke('scan_all_chapters');
}

export async function detectEntities(minCount = 3) {
  return invoke('detect_entities', { minCount });
}

export async function addIgnoredWord(word) {
  return invoke('add_ignored_word', { word });
}

export async function removeIgnoredWord(word) {
  return invoke('remove_ignored_word', { word });
}

// --- Entity notes ---

export async function getEntityNotes(entityId) {
  return invoke('get_entity_notes', { entityId });
}

export async function addEntityNote(entityId, chapterId, yStart, yEnd) {
  return invoke('add_entity_note', { entityId, chapterId: chapterId || null, yStart, yEnd });
}

export async function deleteEntityNote(id) {
  return invoke('delete_entity_note', { id });
}

export async function reorderEntityNotes(ids) {
  return invoke('reorder_entity_notes', { ids });
}

export async function getEntityFreeNotes(entityId) {
  return invoke('get_entity_free_notes', { entityId });
}

export async function addEntityFreeNote(entityId, title, text) {
  return invoke('add_entity_free_note', { entityId, title, text });
}

export async function updateEntityFreeNote(id, title, text) {
  return invoke('update_entity_free_note', { id, title, text });
}

export async function moveEntityFreeNote(id, newEntityId) {
  return invoke('move_entity_free_note', { id, newEntityId });
}

export async function deleteEntityFreeNote(id) {
  return invoke('delete_entity_free_note', { id });
}

export async function reorderEntityFreeNotes(ids) {
  return invoke('reorder_entity_free_notes', { ids });
}

// ---- Chapter planning notes (kanban) ----

export async function getChapterPlanningNotes(chapterId) {
  return invoke('get_chapter_planning_notes', { chapterId });
}

export async function getAllChapterPlanningNotes() {
  return invoke('get_all_chapter_planning_notes');
}

export async function addChapterPlanningNote(chapterId, title, description) {
  return invoke('add_chapter_planning_note', { chapterId, title, description });
}

export async function updateChapterPlanningNote(id, title, description) {
  return invoke('update_chapter_planning_note', { id, title, description });
}

export async function deleteChapterPlanningNote(id) {
  return invoke('delete_chapter_planning_note', { id });
}

export async function reorderChapterPlanningNotes(ids) {
  return invoke('reorder_chapter_planning_notes', { ids });
}

export async function moveChapterPlanningNote(id, newChapterId) {
  return invoke('move_chapter_planning_note', { id, newChapterId });
}

// ---- Kanban freeform board ----

export async function getKanbanColumns() {
  return invoke('get_kanban_columns');
}

export async function addKanbanColumn(title) {
  return invoke('add_kanban_column', { title });
}

export async function updateKanbanColumn(id, title) {
  return invoke('update_kanban_column', { id, title });
}

export async function deleteKanbanColumn(id) {
  return invoke('delete_kanban_column', { id });
}

export async function reorderKanbanColumns(ids) {
  return invoke('reorder_kanban_columns', { ids });
}

export async function getAllKanbanCards() {
  return invoke('get_all_kanban_cards');
}

export async function addKanbanCard(columnId, title, description) {
  return invoke('add_kanban_card', { columnId, title, description });
}

export async function updateKanbanCard(id, title, description) {
  return invoke('update_kanban_card', { id, title, description });
}

export async function deleteKanbanCard(id) {
  return invoke('delete_kanban_card', { id });
}

export async function moveKanbanCard(id, newColumnId, newSortOrder) {
  return invoke('move_kanban_card', { id, newColumnId, newSortOrder });
}

export async function reorderKanbanCards(ids) {
  return invoke('reorder_kanban_cards', { ids });
}

// ---- Semantic search ----

export async function semanticSearch(query, granularity = 'sentence', topN = 20) {
  return invoke('semantic_search', { query, granularity, topN });
}

export async function markChapterDirty(chapterId) {
  return invoke('mark_chapter_dirty', { chapterId });
}

export async function runSemanticIndexing() {
  return invoke('run_semantic_indexing');
}

export async function rebuildSemanticIndex() {
  return invoke('rebuild_semantic_index');
}

// ---- Backup settings ----

export async function getBackupInterval() {
  return invoke('get_backup_interval');
}

export async function setBackupInterval(minutes) {
  return invoke('set_backup_interval', { minutes });
}

export async function getSemanticIndexStatus() {
  return invoke('get_semantic_index_status');
}

export async function textSearch(query, caseSensitive = false, wholeWord = false, useRegex = false, fuzzy = false) {
  return invoke('text_search', { query, caseSensitive, wholeWord, useRegex, fuzzy });
}

export async function wordFrequency(minLength = 4, minCount = 2, windowSize = null) {
  return invoke('word_frequency', { minLength, minCount, windowSize });
}

export async function chapterAnalysis() {
  return invoke('chapter_analysis');
}

export async function pacingAnalysis() {
  return invoke('pacing_analysis');
}

export async function adverbAnalysis() {
  return invoke('adverb_analysis');
}

export async function readabilityAnalysis() {
  return invoke('readability_analysis');
}

export async function paragraphLengthAnalysis() {
  return invoke('paragraph_length_analysis');
}

export async function getChapterDialogue(chapterId) {
  return invoke('get_chapter_dialogue', { chapterId });
}

export async function extractDialogueInText(text) {
  return invoke('extract_dialogue_in_text', { text });
}

export async function debugDialogueSpans(chapterId, aroundPos) {
  return invoke('debug_dialogue_spans', { chapterId, aroundPos });
}

export async function generateHeatmap(entityIds) {
  return invoke('generate_heatmap', { entityIds });
}

export async function findSimilarPhrases(minWords = 5, minSimilarity = 0.6) {
  return invoke('find_similar_phrases', { minWords, minSimilarity });
}

export async function dialogueSearch(query, caseSensitive = false, wholeWord = false, useRegex = false, fuzzy = false) {
  return invoke('dialogue_search', { query, caseSensitive, wholeWord, useRegex, fuzzy });
}

export async function relationshipSearch(entityAId, entityBId, searchType, maxDistance) {
  return invoke('relationship_search', { entityAId, entityBId, searchType, maxDistance });
}

// --- Writing stats ---

export async function logWritingActivity(chapterId, chapterWords, manuscriptWords, wordsDelta) {
  return invoke('log_writing_activity', { chapterId: chapterId || null, chapterWords, manuscriptWords, wordsDelta });
}

export async function getDailyStats(days = 365) {
  return invoke('get_daily_stats', { days });
}

export async function getAllDailyStats() {
  return invoke('get_all_daily_stats');
}

export async function getWritingSettings() {
  return invoke('get_writing_settings');
}

export async function updateWritingSettings(dailyGoal, sessionGapMinutes) {
  return invoke('update_writing_settings', { dailyGoal, sessionGapMinutes });
}

export async function getWritingActivity(date) {
  return invoke('get_writing_activity', { date });
}

export async function getHourlyBreakdown(date) {
  return invoke('get_hourly_breakdown', { date });
}

export async function getManuscriptWordHistory() {
  return invoke('get_manuscript_word_history');
}

export async function debugSearchTerms() {
  return invoke('debug_search_terms');
}

export async function checkWord(word) {
  return invoke('check_word', { word });
}

// --- Navigation history ---

export async function getNavHistory() {
  return invoke('get_nav_history');
}

export async function pushNavEntry(chapterId, scrollTop, cursorPos) {
  return invoke('push_nav_entry', { chapterId, scrollTop: scrollTop || 0, cursorPos: cursorPos || 0 });
}

export async function truncateNavAfter(id) {
  return invoke('truncate_nav_after', { id });
}

export async function findReferences(entityId) {
  return invoke('find_references', { entityId });
}

// --- Spell checker ---

export async function checkSpelling(words) {
  return invoke('check_spelling', { words });
}

export async function getSpellSuggestions(word) {
  return invoke('get_spell_suggestions', { word });
}

export async function addToDictionary(word) {
  return invoke('add_to_dictionary', { word });
}

export async function removeFromDictionary(word) {
  return invoke('remove_from_dictionary', { word });
}

export async function getCustomWords() {
  return invoke('get_custom_words');
}

export async function setSpellLanguage(language) {
  return invoke('set_spell_language', { language });
}

export async function getSpellLanguage() {
  return invoke('get_spell_language');
}

// --- Synonyms ---

export async function getSynonyms(word) {
  return invoke('get_synonyms', { word });
}

// --- Word Palettes ---

export async function getPalettes() {
  return invoke('get_palettes');
}

export async function createPalette(name, description = null) {
  return invoke('create_palette', { name, description });
}

export async function updatePalette(id, name, description = null) {
  return invoke('update_palette', { id, name, description });
}

export async function deletePalette(id) {
  return invoke('delete_palette', { id });
}

export async function togglePalette(id, active) {
  return invoke('toggle_palette', { id, active });
}

export async function copyPalette(id, newName) {
  return invoke('copy_palette', { id, newName });
}

export async function getWordGroups(paletteId) {
  return invoke('get_word_groups', { paletteId });
}

export async function getWordGroup(id) {
  return invoke('get_word_group', { id });
}

export async function createWordGroup(paletteId, name, description = null) {
  return invoke('create_word_group', { paletteId, name, description });
}

export async function updateWordGroup(id, name, description = null) {
  return invoke('update_word_group', { id, name, description });
}

export async function deleteWordGroup(id) {
  return invoke('delete_word_group', { id });
}

export async function getAllSectionNames() {
  return invoke('get_all_section_names');
}

export async function addSection(groupId, name) {
  return invoke('add_section', { groupId, name });
}

export async function addAllSections(groupId, names) {
  return invoke('add_all_sections', { groupId, names });
}

export async function renameSection(id, name) {
  return invoke('rename_section', { id, name });
}

export async function deleteSection(id) {
  return invoke('delete_section', { id });
}

export async function addWordEntry(groupId, sectionId = null, word) {
  return invoke('add_word_entry', { groupId, sectionId, word });
}

export async function addWordEntries(groupId, sectionId = null, words) {
  return invoke('add_word_entries', { groupId, sectionId, words });
}

export async function removeWordEntry(id) {
  return invoke('remove_word_entry', { id });
}

export async function searchWordGroups(query) {
  return invoke('search_word_groups', { query });
}

export async function searchPaletteEntries(query) {
  return invoke('search_palette_entries', { query });
}

export async function getActiveGroups() {
  return invoke('get_active_groups');
}

// --- Comments / notes ---

export async function getChapterComments(chapterId) {
  return invoke('get_chapter_comments', { chapterId });
}

export async function addComment(chapterId, noteText = '') {
  return invoke('add_comment', { chapterId, noteText });
}

export async function updateComment(id, noteText) {
  return invoke('update_comment', { id, noteText });
}

export async function deleteComment(id) {
  return invoke('delete_comment', { id });
}

// --- Entity state tracking (checkpoint model) ---

export async function getEntityMarkers(entityId) {
  return invoke('get_entity_markers', { entityId });
}

export async function addStateMarker(entityId, chapterId) {
  return invoke('add_state_marker', { entityId, chapterId });
}

export async function updateStateMarkerNote(id, note) {
  return invoke('update_state_marker_note', { id, note: note || '' });
}

export async function deleteStateMarker(id) {
  return invoke('delete_state_marker', { id });
}

export async function getStateMarker(id) {
  return invoke('get_state_marker', { id });
}

export async function addStateMarkerValue(markerId, factKey, factValue) {
  return invoke('add_state_marker_value', { markerId, factKey: factKey || '', factValue: factValue || '' });
}

export async function updateStateMarkerValue(id, factKey, factValue) {
  return invoke('update_state_marker_value', { id, factKey: factKey || '', factValue: factValue || '' });
}

export async function deleteStateMarkerValue(id) {
  return invoke('delete_state_marker_value', { id });
}

export async function addStateMarkerEntityRef(markerId, refEntityId, refActive) {
  return invoke('add_state_marker_entity_ref', { markerId, refEntityId, refActive: refActive !== false });
}

export async function updateStateMarkerEntityRef(id, refActive) {
  return invoke('update_state_marker_entity_ref', { id, refActive });
}

export async function getIncomingEntityRefs(entityId) {
  return invoke('get_incoming_entity_refs', { entityId });
}

export async function getEntityStateKeys(entityId) {
  return invoke('get_entity_state_keys', { entityId });
}

export async function getDistinctStateKeys() {
  return invoke('get_distinct_state_keys');
}

// --- Time sections ---

export async function getChapterTimeSections(chapterId) {
  return invoke('get_chapter_time_sections', { chapterId });
}

export async function getAllTimeSections() {
  return invoke('get_all_time_sections');
}

export async function getTimeSectionOrder() {
  return invoke('get_time_section_order');
}

export async function saveTimeSectionOrder(entries) {
  return invoke('save_time_section_order', { entries });
}

export async function resetTimeSectionOrder() {
  return invoke('reset_time_section_order');
}

export async function resolveEntityStateAt(entityId, targetChapterId, targetSectionIndex = 0) {
  return invoke('resolve_entity_state_at', { entityId, targetChapterId, targetSectionIndex });
}

// --- Debug ---

export async function debugStrippedText(chapterId, start, length) {
  return invoke('debug_stripped_text', { chapterId, start, length });
}

// --- Word counts (from Y.Doc) ---

export async function getChapterWordCount(id) {
  return invoke('get_chapter_word_count', { id });
}

export async function getAllChapterWordCounts() {
  return invoke('get_all_chapter_word_counts');
}

// --- Format profiles & pages ---

export async function getFormatProfiles() {
  return invoke('get_format_profiles');
}

export async function getFormatProfile(id) {
  return invoke('get_format_profile', { id });
}

export async function addFormatProfile(name, targetType, trimWidthIn, trimHeightIn) {
  return invoke('add_format_profile', { name, targetType, trimWidthIn, trimHeightIn });
}

export async function updateFormatProfile(id, name, targetType, trimWidthIn, trimHeightIn, marginTopIn, marginBottomIn, marginOutsideIn, marginInsideIn, fontBody, fontSizePt, lineSpacing) {
  return invoke('update_format_profile', { id, name, targetType, trimWidthIn, trimHeightIn, marginTopIn, marginBottomIn, marginOutsideIn, marginInsideIn, fontBody, fontSizePt, lineSpacing });
}

export async function deleteFormatProfile(id) {
  return invoke('delete_format_profile', { id });
}

export async function seedFormatProfiles() {
  return invoke('seed_format_profiles');
}

export async function getFormatPages() {
  return invoke('get_format_pages');
}

export async function addFormatPage(pageRole, title, position) {
  return invoke('add_format_page', { pageRole, title, position });
}

export async function duplicateFormatProfile(sourceId, newName) {
  return invoke('duplicate_format_profile', { sourceId, newName });
}

export async function pasteFormatProfileSettings(targetId, settings) {
  return invoke('paste_format_profile_settings', { targetId, settings });
}

export async function addPageExclusion(pageId, profileId) {
  return invoke('add_page_exclusion', { pageId, profileId });
}

export async function removePageExclusion(pageId, profileId) {
  return invoke('remove_page_exclusion', { pageId, profileId });
}

export async function listPageExclusions() {
  return invoke('list_page_exclusions');
}

export async function updateFormatPage(id, pageRole, title, content, position, includeIn, verticalAlign = 'top', ebookMetadataTag = '') {
  return invoke('update_format_page', { id, pageRole, title, content, position, includeIn, verticalAlign, ebookMetadataTag });
}

export async function deleteFormatPage(id) {
  return invoke('delete_format_page', { id });
}

export async function reorderFormatPages(ids) {
  return invoke('reorder_format_pages', { ids });
}

export async function updateChapterMetadata(id, title, subtitle, chapterImageId) {
  return invoke('update_chapter_metadata', { id, title, subtitle, chapterImageId });
}

export async function compilePreview(profileId) {
  return invoke('compile_preview', { profileId });
}

export async function exportFormatPdf() {
  return invoke('export_format_pdf');
}

// Run the Rust-side EPUB sanity checker on a byte buffer.
// Returns a (possibly empty) array of { level, code, file, message }.
// Empty = passes sanity checks (not a full epubcheck guarantee).
export async function validateEpubBytes(bytes) {
  const arr = bytes instanceof Uint8Array ? Array.from(bytes) : bytes;
  return invoke('validate_epub_bytes', { bytes: arr });
}

export async function exportEpub(request) {
  return invoke('export_epub', { request });
}

export async function listSystemFonts() {
  return invoke('list_system_fonts');
}

export async function updateProfileCategory(profileId, category, json) {
  return invoke('update_profile_category', { profileId, category, json });
}

// --- Manuscript import (DOCX / EPUB) ---

export async function parseImportFile(path, method = null) {
  return invoke('parse_import_file', { path, method });
}

// --- Project settings (key/value) ---

export async function getProjectSetting(key) {
  return invoke('get_project_setting', { key });
}

export async function setProjectSetting(key, value) {
  return invoke('set_project_setting', { key, value: String(value) });
}

// --- Famous-books library (dev tooling) ---

export async function listLibraryBooks() {
  return invoke('list_library_books');
}

export async function getLibraryBook(id) {
  return invoke('get_library_book', { id });
}

export async function saveLibraryBook(title, author, source, wordCount, analysesJson) {
  return invoke('save_library_book', { title, author, source, wordCount, analysesJson });
}

export async function deleteLibraryBook(id) {
  return invoke('delete_library_book', { id });
}
