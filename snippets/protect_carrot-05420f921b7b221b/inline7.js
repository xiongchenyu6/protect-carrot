
export function load_tutorial_done() {
  try { return globalThis.localStorage?.getItem('protect_carrot_tutorial_done') === '1'; }
  catch (_) { return false; }
}
export function save_tutorial_done(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_tutorial_done', value ? '1' : '0'); }
  catch (_) {}
}
