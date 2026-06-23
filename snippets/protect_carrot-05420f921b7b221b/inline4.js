
export function load_quality() {
  try { return globalThis.localStorage?.getItem('protect_carrot_quality') || ''; }
  catch (_) { return ''; }
}
export function save_quality(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_quality', value); }
  catch (_) {}
}
