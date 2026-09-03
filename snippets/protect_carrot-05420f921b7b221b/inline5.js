
export function load_bestiary_counts() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_bestiary') || '';
  } catch (_) {
    return '';
  }
}
export function save_bestiary_counts(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_bestiary', value);
  } catch (_) {}
}
