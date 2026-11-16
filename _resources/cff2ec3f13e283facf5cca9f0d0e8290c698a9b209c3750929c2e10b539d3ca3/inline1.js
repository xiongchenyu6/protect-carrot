
export function load_progress() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_unlocked') || '';
  } catch (_) {
    return '';
  }
}
export function load_progress_stars() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_stars') || '';
  } catch (_) {
    return '';
  }
}
