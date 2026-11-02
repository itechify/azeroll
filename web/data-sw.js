import { openChain } from './local-mpq.js';
import { store } from './local-data-store.js';
const chains = new Map();
const prefix = new URL('data/', self.registration.scope).pathname;
self.addEventListener('install', e => e.waitUntil(self.skipWaiting()));
self.addEventListener('activate', e => e.waitUntil(self.clients.claim()));
self.addEventListener('message', e => {
  if (e.data?.type !== 'mount') return;
  e.waitUntil((async()=>{
    try {
      const chain=await openChain(e.data.files);
      // Persist Files locally so a browser-terminated service worker can reconstruct its indexes.
      if (e.data.handle) await store(e.source.id, e.data.handle);
      else {
        const id=JSON.stringify(e.data.files.map(f=>[f.name,f.size,f.lastModified]).sort());
        const saved=await store('fallback-files');
        if(saved?.id!==id) await store('fallback-files',{id,files:e.data.files});
        await store(e.source.id,{fallback:id});
      }
      chains.get(e.source.id)?.then(c=>c.dispose());
      chains.set(e.source.id,Promise.resolve(chain));
      e.ports[0].postMessage({ok:true});
    } catch(error) { e.ports[0].postMessage({error:String(error)}); }
  })());
});
async function respond(e) {
  try {
    if (!['GET','HEAD'].includes(e.request.method)) return new Response(null,{status:405});
    if (!chains.has(e.clientId)) chains.set(e.clientId,(async()=> {
      let files=await store(e.clientId);
      if(files?.fallback) {
        const saved=await store('fallback-files');
        if(saved?.id!==files.fallback)throw Error('Local selection changed in another tab. Select your folder again.');
        files=saved.files;
      }
      if (files?.kind==='directory') {
        const handle=files; files=[];
        for await(const entry of handle.values())if(entry.kind==='file' && /\.mpq$/i.test(entry.name))files.push(await entry.getFile());
      }
      if (!files) throw Error('Select your Data folder again.');
      return openChain(files);
    })());
    const chain=await chains.get(e.clientId);
    const name=decodeURIComponent(new URL(e.request.url).pathname.slice(prefix.length)).replaceAll('/','\\');
    const data=name==='__index'?new TextEncoder().encode(JSON.stringify(await chain.list())):await chain.read(name);
    if (data===null) return new Response(null,{status:404});
    const ext=name.split('.').pop().toLowerCase();
    const type=name==='__index'?'application/json':({blp:'image/x-blp',mp3:'audio/mpeg',ttf:'font/ttf',wav:'audio/wav'}[ext]||'application/octet-stream');
    // The native host ignores Range; the client always requests whole decoded files.
    return new Response(e.request.method==='HEAD'?null:data,{headers:{'Content-Type':type,'Cache-Control':'no-store','Content-Length':String(data.length)}});
  } catch(error) { console.error('local MPQ:',error); return new Response(String(error),{status:503}); }
}
self.addEventListener('fetch', e => {
  const url=new URL(e.request.url);
  if (url.origin===location.origin && url.pathname.startsWith(prefix)) e.respondWith(respond(e));
});
