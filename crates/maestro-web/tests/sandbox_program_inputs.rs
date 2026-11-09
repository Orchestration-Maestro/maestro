//! Supplied programs and provider runtimes the sandbox witnesses prepare and compile.

/// A provider runtime defining one marker-looking literal.
pub(crate) const MARKER_PROVIDER: &str = r"function(id){window.S={};S.literal='__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__';window.done=new Promise(r=>window.complete=async(error,value)=>{S.error=error;S.value=value;r()});}";
/// User code returning every marker-looking value it can read.
pub(crate) const MARKER_USER: &str = r"return [window.sandboxId,window['__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__'],window.attachment.text,S.literal,'__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__'];";
/// A provider runtime observing its own text, data, loading order and an optional policy.
pub(crate) const PROVIDER: &str = r#"function(){window.S={dcl:false,started:false};S.runtime=document.currentScript.textContent;S.raw=String.raw`</ScRiPt> <!--<script> </script> 雪`;S.data=window.obsData;document.addEventListener('DOMContentLoaded',()=>S.dcl=true);window.done=new Promise(r=>window.complete=async(error,value)=>{S.error=error;S.value=value;r()});if(location.hash==='#tt'){const m=document.createElement('meta');m.httpEquiv='Content-Security-Policy';m.content="script-src 'self' 'unsafe-inline'; require-trusted-types-for 'script'; trusted-types 'none'";document.head.append(m);}}"#;
/// User code observing loading order, module resolution and a preserved carriage return.
pub(crate) const USER: &str = "S.started=true;S.ready=document.readyState;S.beforeDcl=!S.dcl;S.metaUrl=import.meta.url;S.base=document.baseURI;S.cr=(()=>{/*\r*/}).toString().includes('\\r');S.helper=(await import('./helper.js')).answer;return String.raw`</ScRiPt> <!--<script> </script> <![CDATA[ ]]> &amp; &lt;x> 雪`;";
