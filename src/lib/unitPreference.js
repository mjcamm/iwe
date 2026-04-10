// Global measurement unit preference (inches or mm).
// Loaded once from settings.json, shared across all formatting components.
// Write-through: calling setUnit() updates both the in-memory state and settings.json.

import { getSettings, saveSettings } from './db.js';

let _unit = 'in'; // 'in' | 'mm'
let _loaded = false;
let _loadPromise = null;
let _listeners = new Set();

export const MM_PER_IN = 25.4;

export function getUnit() {
  return _unit;
}

export async function ensureUnitLoaded() {
  if (_loaded) return _unit;
  if (_loadPromise) return _loadPromise;
  _loadPromise = getSettings().then(s => {
    _unit = s.formatLengthUnit || 'in';
    _loaded = true;
    return _unit;
  });
  return _loadPromise;
}

export async function setUnit(u) {
  _unit = u;
  _loaded = true;
  notify();
  const s = await getSettings();
  s.formatLengthUnit = u;
  await saveSettings(s);
}

export function toDisplay(inches) {
  if (_unit === 'mm') return (inches * MM_PER_IN).toFixed(1);
  return inches.toFixed(3);
}

export function fromDisplay(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0) return null;
  return _unit === 'mm' ? n / MM_PER_IN : n;
}

export function unitLabel() {
  return _unit === 'mm' ? 'mm' : '\u2033'; // ″
}

export function unitStep() {
  return _unit === 'mm' ? '0.5' : '0.05';
}

// Simple subscription so Svelte components can react to changes
export function subscribe(fn) {
  _listeners.add(fn);
  return () => _listeners.delete(fn);
}

function notify() {
  for (const fn of _listeners) fn(_unit);
}
