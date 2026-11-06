//! Browser-native painting and chart-completion observations.

/// Observe every histogram completion using the chart's own animation callback.
pub(super) const RENDERS: &str = r"(() => {
  window.__done = {}; window.__waiters = [];
  const finishing = canvas => {
    const id = canvas && canvas.id; if (!id) return;
    window.__done[id] = (window.__done[id] || 0) + 1;
    for (const waiter of window.__waiters.slice()) waiter();
  };
  window.__settled = (id, after) => new Promise(resolve => {
    const done = () => (window.__done[id] || 0) > after;
    if (done()) return resolve();
    window.__waiters.push(() => { if (done()) resolve(); });
  });
  let chart;
  Object.defineProperty(window, 'Chart', {
    configurable:true, get:()=>chart,
    set:value => { chart = new Proxy(value, {construct(target,args) {
      const config = args[1] || (args[1] = {});
      const options = config.options || (config.options = {});
      options.animation = {...options.animation, onComplete:()=>finishing(args[0])};
      return new target(...args);
    }}); }
  });
})()";

/// Measure actual canvas pixels and typography against the loaded brand roles.
pub(super) const PROBE: &str = r"(() => {
  const role = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const channels = value => { const hex=value.replace('#',''); return [0,2,4].map(i=>parseInt(hex.slice(i,i+2),16)); };
  const painted = (canvas,[red,green,blue],{fromX=0,toX=1,fromY=0,toY=1}) => {
    const {width,height}=canvas, left=Math.floor(width*fromX),top=Math.floor(height*fromY);
    const pixels=canvas.getContext('2d').getImageData(left,top,Math.floor(width*toX)-left,Math.floor(height*toY)-top).data;
    let count=0;
    for(let i=0;i<pixels.length;i+=4) if(pixels[i]===red&&pixels[i+1]===green&&pixels[i+2]===blue&&pixels[i+3]===255) count++;
    return count;
  };
  window.probe = {
    roles:Object.fromEntries(['accent','muted','border','text','background'].map(name=>[name,channels(role('--maestro-color-'+name))])),
    painted:(name,colour,band)=>painted(document.querySelector(name),colour,band),
    summary:()=>document.getElementById('toolSummary').textContent,
    bitmap:name=>document.querySelector(name).toDataURL(),
    measure:(name,font)=>{const element=document.querySelector(name),context=document.createElement('canvas').getContext('2d');context.font=font||getComputedStyle(element).font;return context.measureText(element.textContent).width;},
    chartTitle:(name,text)=>{
      const canvas=document.querySelector(name),{width,height}=canvas;
      const pixels=canvas.getContext('2d').getImageData(0,0,width,height).data;
      const paintedRow=row=>{let count=0;for(let x=0;x<width;x++) if(pixels[(row*width+x)*4+3]>0) count++;return count;};
      let bottom=height-1;while(bottom>=0&&paintedRow(bottom)===0) bottom--;
      let top=bottom;while(top>0&&paintedRow(top-1)>0) top--;
      let left=width,right=-1;
      for(let y=top;y<=bottom;y++) for(let x=0;x<width;x++) if(pixels[(y*width+x)*4+3]!==0){left=Math.min(left,x);right=Math.max(right,x);}
      const measure=document.createElement('canvas').getContext('2d');measure.font=`normal 12px ${role('--maestro-font-body')}`;
      return {ink:right<left?0:right-left+1,declared:measure.measureText(text).width*devicePixelRatio};
    },
    path:()=>{const code=document.querySelector('main > p > code');return {text:code.textContent,images:code.querySelectorAll('img').length};},
    payload:()=>globalThis.payloadExecuted===undefined,
    payloadName:()=>[...document.querySelectorAll('#tools tbody tr td:first-child')].map(cell=>cell.textContent).find(text=>text.includes('payloadExecuted'))||''
  };
})()";
