<script>
  import { onMount, onDestroy } from 'svelte';
  import {
    getProjectSetting, setProjectSetting,
    getBookCover, setBookCover, clearBookCover, coverToObjectUrl,
  } from '$lib/db.js';
  import { addToast } from '$lib/toast.js';

  // Book metadata fields — stored as individual keys in project_settings table.
  // These are pulled by the formatting system for headers/footers/title pages.
  let bookTitle = $state('');
  let authorName = $state('');
  let subtitle = $state('');
  let seriesName = $state('');
  let seriesNumber = $state('');
  let publisher = $state('');
  let isbn = $state('');
  let copyrightYear = $state('');
  let language = $state('');
  let description = $state('');
  let loading = $state(true);

  // BCP 47 language codes for the Language picklist. Order: English variants
  // first (most common for self-publishing), then the main European languages,
  // then wider Asia / MENA / others, alphabetised within each group.
  const LANGUAGE_OPTIONS = [
    { code: 'en',    label: 'English' },
    { code: 'en-US', label: 'English (United States)' },
    { code: 'en-GB', label: 'English (United Kingdom)' },
    { code: 'en-AU', label: 'English (Australia)' },
    { code: 'en-CA', label: 'English (Canada)' },
    { code: 'en-NZ', label: 'English (New Zealand)' },
    { code: 'fr',    label: 'French' },
    { code: 'fr-CA', label: 'French (Canada)' },
    { code: 'de',    label: 'German' },
    { code: 'es',    label: 'Spanish' },
    { code: 'es-MX', label: 'Spanish (Latin America)' },
    { code: 'it',    label: 'Italian' },
    { code: 'pt',    label: 'Portuguese' },
    { code: 'pt-BR', label: 'Portuguese (Brazil)' },
    { code: 'nl',    label: 'Dutch' },
    { code: 'pl',    label: 'Polish' },
    { code: 'sv',    label: 'Swedish' },
    { code: 'da',    label: 'Danish' },
    { code: 'no',    label: 'Norwegian' },
    { code: 'fi',    label: 'Finnish' },
    { code: 'is',    label: 'Icelandic' },
    { code: 'cs',    label: 'Czech' },
    { code: 'sk',    label: 'Slovak' },
    { code: 'hu',    label: 'Hungarian' },
    { code: 'ro',    label: 'Romanian' },
    { code: 'el',    label: 'Greek' },
    { code: 'ru',    label: 'Russian' },
    { code: 'uk',    label: 'Ukrainian' },
    { code: 'tr',    label: 'Turkish' },
    { code: 'ar',    label: 'Arabic' },
    { code: 'he',    label: 'Hebrew' },
    { code: 'hi',    label: 'Hindi' },
    { code: 'bn',    label: 'Bengali' },
    { code: 'id',    label: 'Indonesian' },
    { code: 'ms',    label: 'Malay' },
    { code: 'th',    label: 'Thai' },
    { code: 'vi',    label: 'Vietnamese' },
    { code: 'ja',    label: 'Japanese' },
    { code: 'ko',    label: 'Korean' },
    { code: 'zh',    label: 'Chinese' },
    { code: 'zh-Hans', label: 'Chinese (Simplified)' },
    { code: 'zh-Hant', label: 'Chinese (Traditional)' },
  ];

  const FIELDS = [
    { key: 'book_title',      label: 'Book Title',       bind: () => bookTitle,      set: v => bookTitle = v,     placeholder: 'The title of your book' },
    { key: 'author_name',     label: 'Author Name',      bind: () => authorName,     set: v => authorName = v,    placeholder: 'Your name as it appears on the cover' },
    { key: 'subtitle',        label: 'Subtitle',         bind: () => subtitle,       set: v => subtitle = v,      placeholder: 'Optional subtitle' },
    { key: 'series_name',     label: 'Series Name',      bind: () => seriesName,     set: v => seriesName = v,    placeholder: 'e.g. "The Dark Tower"' },
    { key: 'series_number',   label: 'Book Number',      bind: () => seriesNumber,   set: v => seriesNumber = v,  placeholder: 'e.g. "1" or "Book Three"' },
    { key: 'publisher',       label: 'Publisher',         bind: () => publisher,      set: v => publisher = v,     placeholder: 'Publishing imprint or self-published' },
    { key: 'isbn',            label: 'ISBN',              bind: () => isbn,           set: v => isbn = v,          placeholder: '978-0-000000-00-0' },
    { key: 'copyright_year',  label: 'Copyright Year',   bind: () => copyrightYear,  set: v => copyrightYear = v, placeholder: new Date().getFullYear().toString() },
    { key: 'language',        label: 'Language',          bind: () => language,       set: v => language = v,      type: 'select', options: LANGUAGE_OPTIONS },
    { key: 'description',     label: 'Description',       bind: () => description,    set: v => description = v,   type: 'textarea', placeholder: 'Short book blurb (used in EPUB metadata)' },
  ];

  // ---- Book cover (BLOB) ----
  let coverUrl = $state(null);
  let coverMime = $state(null);
  let coverUploading = $state(false);
  let coverInput;

  async function loadCover() {
    const cover = await getBookCover();
    if (coverUrl) URL.revokeObjectURL(coverUrl);
    coverUrl = coverToObjectUrl(cover);
    coverMime = cover?.mime_type || null;
  }

  async function handleCoverFile(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!file.type.startsWith('image/')) {
      addToast('Cover must be an image file', 'error');
      return;
    }
    coverUploading = true;
    try {
      const buf = await file.arrayBuffer();
      const bytes = new Uint8Array(buf);
      await setBookCover(bytes, file.type);
      await loadCover();
      addToast('Cover updated', 'success');
    } catch (err) {
      console.error('[book-settings] cover upload failed:', err);
      addToast('Cover upload failed: ' + err, 'error');
    } finally {
      coverUploading = false;
      if (e.target) e.target.value = '';
    }
  }

  async function handleRemoveCover() {
    try {
      await clearBookCover();
      if (coverUrl) URL.revokeObjectURL(coverUrl);
      coverUrl = null;
      coverMime = null;
      addToast('Cover removed', 'success');
    } catch (err) {
      console.error('[book-settings] cover remove failed:', err);
      addToast('Cover remove failed: ' + err, 'error');
    }
  }

  onMount(async () => {
    const promises = FIELDS.map(async f => {
      const val = await getProjectSetting(f.key);
      if (val) f.set(val);
    });
    await Promise.all([...promises, loadCover()]);
    loading = false;
  });

  onDestroy(() => {
    if (coverUrl) URL.revokeObjectURL(coverUrl);
  });

  let saveTimers = {};
  function handleInput(field, e) {
    field.set(e.target.value);
    clearTimeout(saveTimers[field.key]);
    saveTimers[field.key] = setTimeout(() => {
      setProjectSetting(field.key, field.bind());
    }, 400);
  }
  // Selects save immediately — no debounce (user picked deliberately).
  function handleSelect(field, e) {
    field.set(e.target.value);
    setProjectSetting(field.key, field.bind());
  }
</script>

{#if loading}
  <div class="settings-loading">
    <div class="loader"></div>
    <p>Loading book settings...</p>
  </div>
{:else}
  <div class="settings-page">
    <div class="settings-card">
      <h2 class="settings-title">Book Settings</h2>
      <p class="settings-desc">
        Metadata about your book. These values are used by the formatting system
        for title pages, headers, footers, and export metadata.
      </p>

      <div class="cover-section">
        <span class="field-label">Front Cover</span>
        <div class="cover-row">
          <div class="cover-thumb" class:empty={!coverUrl}>
            {#if coverUrl}
              <img src={coverUrl} alt="Book cover" />
            {:else}
              <div class="cover-placeholder">
                <i class="bi bi-image"></i>
                <span>No cover</span>
              </div>
            {/if}
          </div>
          <div class="cover-actions">
            <button class="cover-btn primary" onclick={() => coverInput?.click()} disabled={coverUploading}>
              {#if coverUploading}Uploading…{:else if coverUrl}Replace…{:else}Upload cover…{/if}
            </button>
            {#if coverUrl}
              <button class="cover-btn danger" onclick={handleRemoveCover} disabled={coverUploading}>
                Remove
              </button>
            {/if}
            <p class="cover-hint">
              JPEG, PNG, or WebP. Used in the exported EPUB and shown on the project list.
              A 2:3 aspect ratio (e.g. 1600×2400) works well for most ebook stores.
            </p>
          </div>
        </div>
        <input
          type="file"
          accept="image/jpeg,image/png,image/webp"
          bind:this={coverInput}
          onchange={handleCoverFile}
          style="display:none"
        />
      </div>

      <div class="fields">
        {#each FIELDS as field}
          <label class="field">
            <span class="field-label">{field.label}</span>
            {#if field.type === 'select'}
              <select
                class="field-input"
                value={field.bind()}
                onchange={(e) => handleSelect(field, e)}>
                <option value="">— Select —</option>
                {#each field.options as opt}
                  <option value={opt.code}>{opt.label} ({opt.code})</option>
                {/each}
              </select>
            {:else if field.type === 'textarea'}
              <textarea
                class="field-input field-textarea"
                value={field.bind()}
                oninput={(e) => handleInput(field, e)}
                placeholder={field.placeholder}
                rows="3"
              ></textarea>
            {:else}
              <input
                type="text"
                class="field-input"
                value={field.bind()}
                oninput={(e) => handleInput(field, e)}
                placeholder={field.placeholder}
              />
            {/if}
          </label>
        {/each}
      </div>

      <p class="settings-hint">
        <i class="bi bi-info-circle"></i>
        Changes save automatically. These values appear in the Formatting tab's
        header/footer system and can be referenced in format page templates.
      </p>
    </div>
  </div>
{/if}

<style>
  .settings-loading {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; gap: 1rem;
    font-family: var(--iwe-font-ui); color: var(--iwe-text-muted);
  }
  .loader {
    width: 28px; height: 28px;
    border: 3px solid var(--iwe-border); border-top-color: var(--iwe-accent);
    border-radius: 50%; animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .settings-page {
    height: 100%;
    overflow-y: auto;
    padding: 2rem;
    background: var(--iwe-bg-warm);
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .settings-card {
    background: var(--iwe-bg);
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius);
    padding: 2rem 2.5rem;
    max-width: 600px;
    width: 100%;
    box-shadow: 0 2px 12px rgba(0,0,0,0.06);
  }
  .settings-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 1.4rem;
    color: var(--iwe-text);
    margin: 0 0 0.4rem 0;
  }
  .settings-desc {
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    color: var(--iwe-text-muted); line-height: 1.5;
    margin: 0 0 1.5rem 0;
  }
  .fields {
    display: flex; flex-direction: column; gap: 1rem;
  }
  .field {
    display: flex; flex-direction: column; gap: 0.3rem;
  }
  .field-label {
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .field-input {
    padding: 0.55rem 0.75rem;
    font-family: var(--iwe-font-ui); font-size: 0.95rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    transition: border-color 120ms;
  }
  .field-input:focus {
    outline: none;
    border-color: var(--iwe-accent);
  }
  .field-input::placeholder {
    color: var(--iwe-text-muted); opacity: 0.5;
  }
  .field-textarea {
    resize: vertical;
    min-height: 72px;
    font-family: var(--iwe-font-ui);
    line-height: 1.45;
  }
  select.field-input {
    appearance: none;
    -webkit-appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='%236b6560'%3E%3Cpath d='M4 6l4 4 4-4z'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.6rem center;
    background-size: 16px;
    padding-right: 2rem;
    cursor: pointer;
  }

  /* ---- Cover section ---- */
  .cover-section {
    margin-bottom: 1.5rem;
    padding-bottom: 1.25rem;
    border-bottom: 1px solid var(--iwe-border);
  }
  .cover-section .field-label {
    display: block;
    margin-bottom: 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); text-transform: uppercase;
    letter-spacing: 0.04em; font-weight: 600;
  }
  .cover-row {
    display: flex;
    gap: 1.25rem;
    align-items: stretch;
  }
  .cover-thumb {
    flex-shrink: 0;
    width: 110px;
    aspect-ratio: 2 / 3;
    border: 1px solid var(--iwe-border);
    border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg-warm);
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(0,0,0,0.05);
  }
  .cover-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .cover-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    color: var(--iwe-text-muted);
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
  }
  .cover-placeholder i {
    font-size: 1.6rem;
    opacity: 0.5;
  }
  .cover-actions {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    justify-content: center;
  }
  .cover-btn {
    padding: 0.5rem 0.9rem;
    font-family: var(--iwe-font-ui); font-size: 0.82rem;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg); color: var(--iwe-text);
    cursor: pointer;
    transition: background 120ms, border-color 120ms;
    align-self: flex-start;
  }
  .cover-btn:hover:not(:disabled) {
    background: var(--iwe-bg-hover);
  }
  .cover-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .cover-btn.primary {
    background: var(--iwe-accent);
    color: #fff;
    border-color: var(--iwe-accent);
  }
  .cover-btn.primary:hover:not(:disabled) {
    background: var(--iwe-accent-dark, var(--iwe-accent));
    filter: brightness(0.95);
  }
  .cover-btn.danger {
    color: #c0392b;
  }
  .cover-btn.danger:hover:not(:disabled) {
    background: #fbe9e7;
    border-color: #c0392b;
  }
  .cover-hint {
    margin: 0.35rem 0 0 0;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted); line-height: 1.45;
  }
  .settings-hint {
    margin-top: 1.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted); line-height: 1.5;
    display: flex; align-items: flex-start; gap: 0.5rem;
  }
  .settings-hint i { margin-top: 2px; flex-shrink: 0; }
</style>
