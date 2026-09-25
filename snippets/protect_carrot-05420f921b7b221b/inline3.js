
export function load_volume() {
  try { return globalThis.localStorage?.getItem('protect_carrot_volume') || ''; }
  catch (_) { return ''; }
}
export function save_volume(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_volume', value); }
  catch (_) {}
}
