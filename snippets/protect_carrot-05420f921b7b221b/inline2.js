
export function load_hero() {
  try { return globalThis.localStorage?.getItem('protect_carrot_hero') || ''; }
  catch (_) { return ''; }
}
export function save_hero(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_hero', value); }
  catch (_) {}
}
