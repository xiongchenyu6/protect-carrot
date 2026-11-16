
export function bridge_viewport() {
  const canvas=document.querySelector('canvas');
  if (!canvas) return [0,0,1];
  const rect=canvas.getBoundingClientRect();
  return [rect.width,rect.height,Math.round((devicePixelRatio||1)*1000)/1000];
}
export function bridge_language() { const l=window.carrotRequestedLanguage; window.carrotRequestedLanguage=null; return l==='en'?1:l==='zh-CN'?0:2; }
export function initial_touch_mode() { return matchMedia('(pointer: coarse)').matches; }
export function bridge_prepare() { window.carrotPrepareChapter(0); }
export function bridge_status() { const s=window.carrotChapterDownload?.status; return s==='ready'?1:s==='cancelled'?2:0; }
export function bridge_finish() { window.carrotAssetsReady?.(); window.carrotChapterDownload=null; }
export function bridge_release() { window.carrotReleaseChapter?.(); }
export function bridge_downloads() { window.carrotOpenDownloads?.(); }
