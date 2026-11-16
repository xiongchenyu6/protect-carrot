
export function load_brightness() {
  try { return globalThis.localStorage?.getItem('protect_carrot_brightness') || ''; }
  catch (_) { return ''; }
}
export function save_brightness(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_brightness', value); }
  catch (_) {}
}
