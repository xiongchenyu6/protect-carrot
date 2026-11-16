
export function load_equipment_inventory() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_equipment') || '';
  } catch (_) {
    return '';
  }
}
export function save_equipment_inventory(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_equipment', value);
  } catch (_) {}
}
