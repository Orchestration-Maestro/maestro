import * as routes from '../sandbox_loading_support/routes.js';
import {assert,equal,finish} from '../sandbox_loading_support/native.js';
export {finish};
let saved, active, captured, parse,restoreAttribute;
export function begin() {
    routes.begin();
    saved={set:window.setTimeout,clear:window.clearInterval}; active=new Set(); captured=undefined;
    window.setTimeout=function(callback,delay,...args) {
        if(delay!==120000)return saved.set.call(this,callback,delay,...args);
        const id=saved.set.call(this,callback,delay,...args);active.add(id);captured={callback,delay};return id;
    };
    window.clearInterval=function(id) {active.delete(id);return saved.clear.call(this,id);};
}
export function end() {
    if(restoreAttribute)restoreAttribute();
    timers(0);window.setTimeout=saved.set;window.clearInterval=saved.clear;routes.end();finish();
}
export function timers(count) {equal(active.size,count,'production timer set');}
export function deadline() {equal(captured.delay,120000,'two-minute production callback');captured.callback();}
export function fault(mode) {
    if(restoreAttribute){restoreAttribute();restoreAttribute=undefined;}
    routes.fault(mode);
    if(mode==='attribute'){
        const original=Element.prototype.setAttribute;
        Element.prototype.setAttribute=function(...args){if(this.localName==='iframe')throw new DOMException('controlled attribute','InvalidStateError');return original.apply(this,args);};
        restoreAttribute=()=>Element.prototype.setAttribute=original;
    }
}
export const closed=routes.closed;
export function watch(tag) {return new Promise(resolve=>{const listener=event=>{if(event.data?.type===tag){window.removeEventListener('message',listener);resolve(event.data);}};window.addEventListener('message',listener);});}
export function bootstrap(ready) {
    return 'data:text/html,'+encodeURIComponent(`<script>addEventListener('message',e=>{if(e.data.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();}});parent.postMessage({type:${JSON.stringify(ready?'sandbox-ready':'bootstrap-waiting')}},'*');</script>`);
}
export function dispatch(tag,current) {
    const frame=document.querySelector('[data-sandbox-test] iframe');
    window.dispatchEvent(new MessageEvent('message',{source:current?frame.contentWindow:window,data:{type:tag,error:'bootstrap problem',stack:'native stack'}}));
}
export function parser(mode) {
    parse=DOMParser.prototype.parseFromString;
    DOMParser.prototype.parseFromString=function(...args) {
        if(mode==='throw')throw new Error('controlled parse failure');
        if(mode==='empty-throw')throw {};
        const document=parse.apply(this,args);
        const node=document.createElement('parsererror');node.textContent=mode==='empty'?'':'controlled parser error';document.body.append(node);return document;
    };
}
export function restore_parser() {DOMParser.prototype.parseFromString=parse;}
export function remember_frame() {return document.querySelector('[data-sandbox-test] iframe').contentWindow;}
export function dispatch_source(tag,source) {window.dispatchEvent(new MessageEvent('message',{source,data:{type:tag,error:'stale',stack:'stale'}}));}
export function release_frame(){document.querySelector('[data-sandbox-test] iframe').contentWindow.postMessage('release','*');}
export function release_source(source){source.postMessage('release','*');}
export function assert_native(name,value) {
    switch(name){
        case 'native_null':assert(value===null,name);break;
        case 'native_false':case 'native_true':equal(value,name==='native_true',name);break;
        case 'native_zero':assert(Object.is(value,0),name);break;
        case 'native_negative_zero':assert(Object.is(value,-0),name);break;
        case 'native_small':equal(value,1e-7,name);break;
        case 'native_large':equal(value,1e21,name);break;
        case 'native_empty_string':equal(value,'',name);break;
        case 'native_unicode':equal(value,'雪\uFEFF\u0085',name);break;
        case 'native_object':equal(Object.keys(value),['2','10','z','a'],name);equal(value.a,[true,null],name);break;
        case 'native_array':assert(Array.isArray(value),name);equal(value,[2,null,'x'],name);break;
        case 'native_date':assert(value instanceof Date,name);equal(value.toISOString(),'2020-01-02T03:04:05.000Z',name);break;
        case 'native_map':assert(value instanceof Map,name);equal([...value],[['a',1]],name);break;
        case 'native_set':assert(value instanceof Set,name);equal([...value],[2,1],name);break;
        case 'native_bytes':assert(value instanceof Uint8Array,name);equal([...value],[0,128,255],name);break;
        case 'native_bigint':assert(value===9n,name);break;
        case 'native_nan':assert(Number.isNaN(value),name);break;
        case 'native_infinity':assert(value===Infinity,name);break;
        case 'native_negative_infinity':assert(value===-Infinity,name);break;
        default:throw Error('unconsumed native assertion '+name);
    }
}
