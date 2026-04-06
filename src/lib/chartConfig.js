// Shared chart sizing constants. Used by all per-chapter analysis charts so
// they have a consistent visual rhythm. Tune these and every chart updates.

// Minimum width per chapter "slot" on charts where each chapter is a single bar.
// (Words per chapter, avg sentence length, vocabulary density when no comparison.)
export const MIN_SLOT_WIDTH = 40;

// Minimum width per chapter "slot" when the slot contains a paired group
// (manuscript + comparison side by side).
export const MIN_PAIR_SLOT_WIDTH = 70;

// Minimum width per chapter "slot" when the slot contains 4 bars
// (narrative + dialogue × 2 books).
export const MIN_QUAD_SLOT_WIDTH = 110;

// Maximum width for any single bar to keep proportions sensible on wide screens.
export const MAX_BAR_WIDTH = 60;
