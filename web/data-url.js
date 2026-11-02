export function hostedBase() {
  return typeof window.__azeroll_data_base==='string' ? window.__azeroll_data_base.trim().replace(/\/$/,'') : '';
}
export function dataUrl(name) {
  const base=hostedBase();
  return base ? base+'/'+name.replaceAll('\\','/').toLowerCase().split('/').map(encodeURIComponent).join('/')
    : new URL('./data/'+encodeURIComponent(name),import.meta.url).href;
}
// Run before wasm starts: a failed core fetch can safely switch to the local service worker.
export async function prepareData() {
  let reason='';
  const local=new URLSearchParams(location.search).has('localdata');
  if (!local && hostedBase()) {
    const {installHostedFetch}=await import('./hosted-fetch.js');
    installHostedFetch(hostedBase());
    const status=document.createElement('p');status.textContent='Checking game data…';
    status.style.cssText='position:fixed;inset:40% 10%;color:#ffd25a;text-align:center;font:24px Georgia';document.body.append(status);
    try {
      for (const name of ['__index','DBFilesClient\\Map.dbc','DBFilesClient\\AreaTable.dbc','Interface\\FrameXML\\FrameXML.toc','Fonts\\FRIZQT__.TTF']) {
        const response=await fetch(dataUrl(name),{signal:AbortSignal.timeout(20000)});
        if(!response.ok)throw Error(`${name}: HTTP ${response.status}`);
        const bytes=await response.arrayBuffer();
        if(name==='__index' && !Array.isArray(JSON.parse(new TextDecoder().decode(bytes))))throw Error('Invalid game data index');
      }
      return;
    } catch(e) { reason='Hosted game data is unavailable. Choose your local Data folder to play.'; console.warn('Hosted game data unavailable; choose local files:',e); }
    finally {status.remove();}
  } else if (!local && !location.hostname.endsWith('.github.io')) return;
  window.__azeroll_data_base='';
  const {selectLocalData}=await import('./local-data.js');
  await selectLocalData(reason);
}
