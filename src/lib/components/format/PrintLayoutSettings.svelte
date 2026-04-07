<script>
  let { profile, onchange } = $props();
</script>

<div class="custom-section">
  <h4 class="custom-section-title">Print Layout</h4>
  <p class="custom-placeholder">Margins (top/bottom/inside/outside), bleed, gutter calculation, chapter sink, page numbering (Roman vs Arabic).</p>
</div>

<style>
  .custom-section { padding: 0.4rem 0; }
  .custom-section-title {
    font-family: var(--iwe-font-prose);
    font-weight: 400; font-size: 0.95rem;
    margin: 0 0 0.6rem 0; color: var(--iwe-text);
  }
  .custom-placeholder {
    font-family: var(--iwe-font-ui); font-size: 0.78rem;
    color: var(--iwe-text-muted); font-style: italic;
    margin: 0; line-height: 1.4;
  }
</style>
