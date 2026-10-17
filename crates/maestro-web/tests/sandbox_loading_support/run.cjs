// Run retained WebAssembly browser witnesses against a local deterministic host.
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs=require('node:fs'),http=require('node:http'),path=require('node:path'),assert=require('node:assert/strict');
const root=path.join(process.cwd(),'target/sandbox-browser');
(async()=>{
 const server=http.createServer((req,res)=>{
  res.setHeader('Access-Control-Allow-Origin','*');
  const url=req.url.split('?')[0].split('#')[0];
  if(url==='/') {res.setHeader('Content-Type','text/html');res.end('<!doctype html><body></body>');return;}
  if(url==='/favicon.ico'){res.writeHead(204);res.end();return;}
  if(url.endsWith('/helper.js')){res.setHeader('Content-Type','application/javascript');res.end("export const answer='helper-ok'");return;}
  if(url==='/pixel.png'){res.writeHead(404);res.end();return;}
  if(url==='/sandbox.html'){
   res.setHeader('Content-Type','text/html');
   res.end(`<script>addEventListener('message',e=>{if(e.data?.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();parent.postMessage({type:'sandbox-installed'},'*');}});parent.postMessage({type:'sandbox-ready'},'*');</script>`);return;
  }
  const file=path.resolve(root,'.'+url);
  if(!file.startsWith(root+path.sep)){res.writeHead(403);res.end();return;}
  res.setHeader('Content-Type',url.endsWith('.wasm')?'application/wasm':'application/javascript');
  res.end(fs.readFileSync(file));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const traversal=await new Promise(resolve=>http.get({host:'127.0.0.1',port:server.address().port,path:'/../../Cargo.toml'},response=>{response.resume();resolve(response.statusCode);}));
 if(traversal!==403){server.close();assert.equal(traversal,403,'runner refuses paths outside its generated asset root');}
 console.log('PASS runner asset-root containment');
 const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox']});
 console.log('BROWSER Chromium '+browser.version());
 try {
  for(const name of process.argv.slice(2)){
   const page=await browser.newPage();
   page.on('console',m=>console.log(m.text()));
   await page.exposeBinding('inspectSandbox',async({page},id,action)=>{
    let selected;
    for(const frame of page.frames().slice(1)){
     const element=await frame.frameElement();
     if(await element.evaluate((e,id)=>e.parentElement.id===id,id)){selected=frame;break;}
    }
    if(!selected)throw new Error('mounted frame not found '+id);
    if(action==='click'){await selected.locator('#target').click();return;}
    return selected.evaluate(async()=>{
     if(document.readyState!=='complete')await new Promise(r=>addEventListener('load',r,{once:true}));
     if(window.S?.started)await window.done;
     return {...window.S,head:[...document.head.children].map(e=>e.localName).join(','),body:[...document.body.children].map(e=>e.localName).join(','),scaffold:document.querySelectorAll('svg,template').length,metaParent:document.querySelector('meta[http-equiv]')?.parentElement.localName,origin:self.origin,href:location.href};
    });
   });
   await page.goto(`http://127.0.0.1:${server.address().port}`);
   const failed=new Promise((_,reject)=>{if(process.env.SANDBOX_TEST_MODULE==='sandbox_execution_browser')page.on('pageerror',reject);page.on('console',m=>{if(m.text().includes('panicked at'))reject(new Error(m.text()));});});
   await Promise.race([failed,page.evaluate(async name=>{const m=await import('/'+name.module+'.js');await m.default();await m[name.test]();},{module:process.env.SANDBOX_TEST_MODULE||'sandbox_loading_browser',test:name})]);
   console.log('PASS '+name);await page.close();
  }
 }finally{await browser.close();server.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
