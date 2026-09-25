
export function load_hero_gear() {
  try { return window.localStorage.getItem('protect_carrot_hero_gear') || ''; } catch (e) { return ''; }
}
export function save_hero_gear(value) {
  try { window.localStorage.setItem('protect_carrot_hero_gear', value); } catch (e) {}
}
