<script>
  /**
   * Reusable decimal number input. Uses type="text" so the browser doesn't
   * fight the user on decimal placement. Validates and commits on blur or Enter.
   *
   * Props:
   *   value     — current numeric value (canonical, e.g. inches)
   *   onchange  — called with the new numeric value when the user commits
   *   min       — minimum allowed value (default 0)
   *   max       — maximum allowed value (default Infinity)
   *   decimals  — decimal places to display (default 3)
   *   suffix    — unit label shown inside the input (e.g. "in", "mm")
   *   step      — increment for up/down arrow keys (default 0.0625)
   *   disabled  — disable the input
   */
  let {
    value = 0,
    onchange,
    min = 0,
    max = Infinity,
    decimals = 3,
    suffix = '',
    step = 0.0625,
    disabled = false,
  } = $props();

  // Display string — updated from value prop, edited freely by the user
  let display = $state(formatValue(value));

  // Track whether the input is focused — don't overwrite user's in-progress edits
  let focused = $state(false);

  // Sync display when value prop changes externally (but not while user is editing)
  $effect(() => {
    if (!focused) {
      display = formatValue(value);
    }
  });

  function formatValue(v) {
    if (v == null || isNaN(v)) return '0';
    // Trim trailing zeros but keep at least one decimal
    const s = Number(v).toFixed(decimals);
    // Remove unnecessary trailing zeros: 0.250 → 0.25, 1.000 → 1
    return parseFloat(s).toString();
  }

  function parseAndClamp(text) {
    const trimmed = text.trim().replace(/,/g, '.');
    if (trimmed === '' || trimmed === '-') return null;
    const num = parseFloat(trimmed);
    if (isNaN(num)) return null;
    return Math.min(max, Math.max(min, num));
  }

  function commit() {
    const parsed = parseAndClamp(display);
    if (parsed != null && parsed !== value) {
      onchange?.(parsed);
    }
    // Always reformat to clean display, even if value didn't change
    display = formatValue(parsed ?? value);
  }

  function handleBlur() {
    focused = false;
    commit();
  }

  function handleKeydown(e) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit();
      e.target.blur();
    } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const current = parseAndClamp(display) ?? value;
      const delta = e.key === 'ArrowUp' ? step : -step;
      const next = Math.min(max, Math.max(min, current + delta));
      display = formatValue(next);
      onchange?.(next);
    } else if (e.key === 'Escape') {
      display = formatValue(value);
      e.target.blur();
    }
  }

  function handleFocus(e) {
    focused = true;
    // Select all on focus for easy replacement
    e.target.select();
  }
</script>

<div class="decimal-input-wrap" class:disabled>
  <input
    type="text"
    inputmode="decimal"
    bind:value={display}
    onblur={handleBlur}
    onfocus={handleFocus}
    onkeydown={handleKeydown}
    {disabled}
  />
  {#if suffix}
    <span class="decimal-input-suffix">{suffix}</span>
  {/if}
</div>

<style>
  .decimal-input-wrap {
    display: flex; align-items: center;
    border: 1px solid var(--iwe-border); border-radius: var(--iwe-radius-sm);
    background: var(--iwe-bg);
    overflow: hidden;
  }
  .decimal-input-wrap:focus-within { border-color: var(--iwe-accent); }
  .decimal-input-wrap.disabled { opacity: 0.5; pointer-events: none; }
  .decimal-input-wrap input {
    flex: 1; min-width: 0;
    border: none; background: none;
    padding: 0.4rem 0.5rem;
    font-family: var(--iwe-font-ui); font-size: 0.85rem;
    color: var(--iwe-text);
    outline: none;
  }
  .decimal-input-suffix {
    padding: 0 0.55rem;
    font-family: var(--iwe-font-ui); font-size: 0.72rem;
    color: var(--iwe-text-muted);
    background: var(--iwe-bg-warm);
    border-left: 1px solid var(--iwe-border);
    height: 100%;
    display: flex; align-items: center;
    user-select: none;
  }
</style>
