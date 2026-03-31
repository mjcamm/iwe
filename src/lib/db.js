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

export async function addEntityNote(entityId, chapterId, text) {
  return invoke('add_entity_note', { entityId, chapterId: chapterId || null, text });
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

export async function addEntityFreeNote(entityId, text) {
  return invoke('add_entity_free_note', { entityId, text });
}

export async function updateEntityFreeNote(id, text) {
  return invoke('update_entity_free_note', { id, text });
}

export async function deleteEntityFreeNote(id) {
  return invoke('delete_entity_free_note', { id });
}

export async function reorderEntityFreeNotes(ids) {
  return invoke('reorder_entity_free_notes', { ids });
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
