<script>
  import { onMount } from 'svelte';
  import { getProjectSetting, setProjectSetting } from '$lib/db.js';

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
  let loading = $state(true);

  const FIELDS = [
    { key: 'book_title',      label: 'Book Title',       bind: () => bookTitle,      set: v => bookTitle = v,     placeholder: 'The title of your book' },
    { key: 'author_name',     label: 'Author Name',      bind: () => authorName,     set: v => authorName = v,    placeholder: 'Your name as it appears on the cover' },
    { key: 'subtitle',        label: 'Subtitle',         bind: () => subtitle,       set: v => subtitle = v,      placeholder: 'Optional subtitle' },
    { key: 'series_name',     label: 'Series Name',      bind: () => seriesName,     set: v => seriesName = v,    placeholder: 'e.g. "The Dark Tower"' },
    { key: 'series_number',   label: 'Book Number',      bind: () => seriesNumber,   set: v => seriesNumber = v,  placeholder: 'e.g. "1" or "Book Three"' },
    { key: 'publisher',       label: 'Publisher',         bind: () => publisher,      set: v => publisher = v,     placeholder: 'Publishing imprint or self-published' },
    { key: 'isbn',            label: 'ISBN',              bind: () => isbn,           set: v => isbn = v,          placeholder: '978-0-000000-00-0' },
    { key: 'copyright_year',  label: 'Copyright Year',   bind: () => copyrightYear,  set: v => copyrightYear = v, placeholder: new Date().getFullYear().toString() },
  ];

  onMount(async () => {
    const promises = FIELDS.map(async f => {
      const val = await getProjectSetting(f.key);
      if (val) f.set(val);
    });
    await Promise.all(promises);
    loading = false;
  });

  let saveTimers = {};
  function handleInput(field, e) {
    field.set(e.target.value);
    clearTimeout(saveTimers[field.key]);
    saveTimers[field.key] = setTimeout(() => {
      setProjectSetting(field.key, field.bind());
    }, 400);
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

      <div class="fields">
        {#each FIELDS as field}
          <label class="field">
            <span class="field-label">{field.label}</span>
            <input
              type="text"
              class="field-input"
              value={field.bind()}
              oninput={(e) => handleInput(field, e)}
              placeholder={field.placeholder}
            />
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
  .settings-hint {
    margin-top: 1.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.75rem;
    color: var(--iwe-text-muted); line-height: 1.5;
    display: flex; align-items: flex-start; gap: 0.5rem;
  }
  .settings-hint i { margin-top: 2px; flex-shrink: 0; }
</style>
