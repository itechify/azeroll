import { openChain } from './local-mpq.js';
import { store } from './local-data-store.js';
export async function selectLocalData(reason = '') {
  if (!navigator.serviceWorker) throw Error('Local game data requires HTTPS and a browser with service workers.');
  const root=document.createElement('section');
  root.id='azeroll-data';
  root.innerHTML=`<style>
  #azeroll-data{position:fixed;inset:0;z-index:30;display:grid;place-items:center;background:#100c08;color:#e9d8af;font:18px/1.6 Georgia,serif;padding:24px;box-sizing:border-box}
  #azeroll-data [hidden]{display:none} #azeroll-data .content{max-width:620px} #azeroll-data h1{font-size:clamp(38px,8vw,72px);line-height:1.1;color:#ffd25a;margin:0 0 24px}
  #azeroll-data a{color:#ffdf8c;text-underline-offset:3px} #azeroll-data h2{font-size:26px;line-height:1.3;font-weight:normal} #azeroll-data button,#azeroll-data label{font:inherit;color:#ffdf8c;background:#35250f;border:1px solid #987335;padding:12px 20px;cursor:pointer;display:inline-block;margin:8px 12px 8px 0}
  #azeroll-data button:hover,#azeroll-data label:hover{background:#4a3316} #azeroll-data :focus-visible{outline:3px solid #ffe9a3;outline-offset:4px} #azeroll-data button:disabled{opacity:.6;cursor:wait}
  #azeroll-data input{display:block;max-width:100%;font:14px system-ui;margin:12px 0} #azeroll-data .status{color:#ffbc9d;min-height:2em} #azeroll-data small{display:block;color:#c6b58f}
  </style><div class="content"><h1>Azeroll</h1><h2>Point Azeroll at your World of Warcraft 1.12.1 Data folder</h2>
  <p>Bring your own build 5875 install. Your game files are read locally and never uploaded.</p>
  <p>Don't have the client? <a href="https://archive.org/details/WoW_1.12.1-5875" target="_blank" rel="noopener noreferrer">Download 1.12.1-5875_enUS.zip</a> (about 5.4 GB), unzip it, then select its <code>Data</code> folder.</p>
  <button id="pick-data">Choose Data folder</button><button id="resume-data" hidden>Use saved folder</button>
  <details><summary>Choose files with the alternative folder picker</summary><input id="data-files" type="file" webkitdirectory multiple aria-label="World of Warcraft Data folder"></details>
  <p class="status" role="status" aria-live="polite"></p><small>Chrome or Edge with WebGPU required. Once loaded, use W A S D to roll.</small></div>`;
  document.body.append(root);
  const status=root.querySelector('.status');
  status.textContent=reason;
  const registration=await navigator.serviceWorker.register(new URL('./data-sw.js',import.meta.url),{type:'module',scope:new URL('./',import.meta.url).pathname});
  await navigator.serviceWorker.ready;
  if (!navigator.serviceWorker.controller) await new Promise(resolve=>navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));
  const saved=await store('directory').catch(()=>null);
  const resume=root.querySelector('#resume-data'); resume.hidden=!saved;
  await new Promise(resolve=>{
    let busy=false;
    async function mount(files,handle) {
      if(busy)return; busy=true; status.textContent='Checking your local archives…';
      root.querySelectorAll('button').forEach(b=>b.disabled=true);
      try {
        // Only root-level MPQs belong to this chain; ignore backups and locale subdirectories.
        files=files.filter(f=>/\.mpq$/i.test(f.name));
        await new Promise((ok,no)=>{
          const ch=new MessageChannel();
          ch.port1.onmessage=e=>{ch.port1.close();e.data.ok?ok():no(Error(e.data.error));};
          registration.active.postMessage({type:'mount',files,handle},[ch.port2]);
        });
        const chain=await openChain(files);
        const index=new TextEncoder().encode(JSON.stringify(await chain.list()));
        window.__azeroll_read_local=name=>name==='__index'?index:chain.readSync(name);
        if(handle) await store('directory',handle).catch(()=>{});
        root.remove(); resolve();
      } catch(e) { status.textContent=`${e.message} Choose a different folder to retry.`; }
      finally { busy=false;root.querySelectorAll('button').forEach(b=>b.disabled=false); }
    }
    async function directory(handle) {
      try {
        if(await handle.requestPermission({mode:'read'})!=='granted') throw Error('Folder access was denied.');
        const files=[];for await(const entry of handle.values())if(entry.kind==='file')files.push(await entry.getFile());
        await mount(files,handle);
      } catch(e) { if(e.name!=='AbortError')status.textContent=e.message; }
    }
    root.querySelector('#pick-data').onclick=async()=>{
      if(!window.showDirectoryPicker){root.querySelector('details').open=true;root.querySelector('input').click();return;}
      try{await directory(await showDirectoryPicker({mode:'read'}));}catch(e){if(e.name!=='AbortError')status.textContent=e.message;}
    };
    resume.onclick=()=>directory(saved);
    root.querySelector('input').onchange=e=>mount([...e.target.files].filter(f=>f.webkitRelativePath.split('/').length===2));
  });
}
