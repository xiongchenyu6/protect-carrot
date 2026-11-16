// Real launcher/SW UI with a tiny engine fixture; actual WebGPU gameplay is verified separately.
const {chromium}=require('playwright');
const fs=require('fs'),path=require('path'),os=require('os'),http=require('http'),cp=require('child_process'),zlib=require('zlib'),assert=require('assert/strict');
(async()=>{
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'carrot-launcher-')),site=path.join(temp,'site');fs.mkdirSync(site);
 for(const name of ['index.html','launcher.js','launcher.css','game-cache.js','game-cache.css','downloads.js','pwa.js','pwa.css','manifest.webmanifest','icons','favicon.ico'])fs.cpSync(path.join('web',name),path.join(site,name),{recursive:true});
 fs.writeFileSync(path.join(site,'protect_carrot.js'),'export default async function(){window.__carrotReady();}');
 fs.writeFileSync(path.join(site,'protect_carrot_bg.wasm'),'fixture');fs.writeFileSync(path.join(site,'protect_carrot_bg.wasm.gz'),zlib.gzipSync('fixture'));
 fs.mkdirSync(path.join(site,'assets/pixelfx'),{recursive:true});fs.writeFileSync(path.join(site,'assets/pixelfx/battle.webp'),'battle');
 cp.execFileSync('python3',['tools/generate_web_manifest.py',site,'--revision','fixture','--template','tools/sw-template.js']);
 const requests=[];const server=http.createServer((req,res)=>{requests.push(req.url);let file=path.join(site,decodeURIComponent(req.url.split('?')[0]));if(req.url.split('?')[0].endsWith('/'))file=path.join(file,'index.html');try{const ext=path.extname(file);res.setHeader('Content-Type',({'.js':'text/javascript','.css':'text/css','.html':'text/html','.json':'application/json','.webmanifest':'application/manifest+json'})[ext]||'application/octet-stream');res.end(fs.readFileSync(file));}catch{res.statusCode=404;res.end();}});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_EXECUTABLE_PATH});const context=await browser.newContext({viewport:{width:667,height:375},isMobile:true,hasTouch:true,userAgent:'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/130.0.0.0 Mobile Safari/537.36'});
 await context.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{},configurable:true}));
 const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(`http://127.0.0.1:${server.address().port}/`);
 await page.locator('[data-language="en"]').click();assert.equal(await page.locator('#title').textContent(),'Protect the Carrot');await page.reload();assert.equal(await page.locator('#start').textContent(),'Start game');
 await page.locator('[data-language="zh-CN"]').click();assert.equal(await page.locator('#start').textContent(),'开始游戏');
 await page.locator('#install-button').click();await page.locator('#install-dialog[open]').waitFor();assert.ok((await page.locator('#install-steps').textContent()).length>10);await page.locator('#install-close').click();
 await page.locator('#start').click();await page.locator('#enter').waitFor({state:'visible',timeout:30000});assert.equal(await page.locator('#status').textContent(),'准备就绪');assert.ok(!requests.some(x=>x.includes('battle.webp')),'core must not fetch combat');
 await page.locator('#enter').click();await page.evaluate(()=>window.carrotPrepareChapter(0));await page.locator('#downloads-confirm:not([disabled])').waitFor();await page.locator('#downloads-close').click();assert.equal(await page.evaluate(()=>window.carrotChapterDownload.status),'cancelled');
 await page.evaluate(()=>window.carrotPrepareChapter(0));await page.locator('#downloads-confirm:not([disabled])').waitFor();await page.locator('#downloads-confirm').click();await page.waitForFunction(()=>window.carrotChapterDownload?.status==='ready');assert.ok(await page.locator('#downloads-dialog').evaluate(d=>d.open),'must wait for decoded combat effects');await page.evaluate(()=>window.carrotAssetsReady());
 await page.evaluate(()=>window.carrotOpenDownloads());await page.locator('#downloads-groups p').first().waitFor();const bounds=await page.locator('#downloads-close').boundingBox();assert.ok(bounds.y+bounds.height<=375,'back control must fit mobile');await page.locator('#downloads-close').click();
 assert.deepEqual(errors,[]);console.log('PASS launcher language before start/persistence, install guide, first-install activation, core-only startup, cancel, decoded-readiness gate, compact mobile back control');
 await browser.close();await new Promise(r=>server.close(r));fs.rmSync(temp,{recursive:true,force:true});
})().catch(e=>{console.error(e);process.exit(1)});
