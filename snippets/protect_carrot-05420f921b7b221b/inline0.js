
export function load_progress() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_unlocked') || '';
  } catch (_) {
    return '';
  }
}
export function save_progress(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_unlocked', value);
  } catch (_) {}
}
export function load_progress_stars() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_stars') || '';
  } catch (_) {
    return '';
  }
}
export function save_progress_stars(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_stars', value);
  } catch (_) {}
}
