
export function load_lang() {
  try { return globalThis.localStorage?.getItem('protect_carrot_lang') || ''; }
  catch (_) { return ''; }
}
export function save_lang(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_lang', value); }
  catch (_) {}
}
