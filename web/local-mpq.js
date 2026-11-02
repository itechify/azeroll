// All reads stay in this origin's service worker; never fetch an asset from the network.
import init, { MpqIndex, extract } from './wenilla_mpq_web.js';
let wasm;
const BASE = ['dbc','speech','fonts','interface','misc','sound','wmo','terrain','texture','model'];
export function mountOrder(files) {
  const byName = new Map(files.map(f => [f.name.toLowerCase(), f]));
  const patches = [...byName.keys()].filter(n => /^patch-.\.mpq$/.test(n)).sort();
  return [...BASE.map(n => n+'.mpq'), 'patch.mpq', ...patches, 'speech2.mpq'].map(n => byName.get(n)).filter(Boolean);
}
async function slice(file, start, length) {
  if (start < 0 || length < 0 || start+length > file.size || length > 128*1024*1024) throw Error(`Invalid archive bounds in ${file.name}`);
  return new Uint8Array(await file.slice(start, start+length).arrayBuffer());
}
export async function openChain(files) {
  await (wasm ??= init());
  for (const name of ['terrain.mpq','patch.mpq','dbc.mpq','model.mpq','texture.mpq']) {
    if (!files.some(f => f.name.toLowerCase() === name)) throw Error(`Missing ${name}. Select the complete World of Warcraft 1.12.1 (build 5875) Data folder.`);
  }
  const archives = [];
  try {
    for (const file of mountOrder(files)) {
      let offset = 0, header;
      while (offset < Math.min(file.size, 1024*1024)) {
        header = new DataView((await slice(file, offset, 32)).buffer);
        if (header.getUint32(0,true) === 0x1b51504d) { offset += header.getUint32(8,true); continue; }
        if (header.getUint32(0,true) === 0x1a51504d) break;
        offset += 512;
      }
      if (header?.getUint32(0,true) !== 0x1a51504d || header.getUint16(12,true)>1 || header.getUint16(14,true)>11) throw Error(`Invalid vanilla MPQ: ${file.name}`);
      const hashes = await slice(file, offset+header.getUint32(16,true), header.getUint32(24,true)*16);
      const blocks = await slice(file, offset+header.getUint32(20,true), header.getUint32(28,true)*16);
      archives.push({file, offset, sector:512*2**header.getUint16(14,true), index:new MpqIndex(hashes,blocks)});
    }
    const chain = {
      dispose() { for (const a of archives) a.index.free(); },
      readSync(name) {
        for(const a of [...archives].reverse()) {
          const b=a.index.lookup(name); if(!b.length)continue;
          const [pos,compressed,size,flags]=b;
          if((flags&0x2000000)||!(flags&0x80000000))return null;
          if(compressed>128*1024*1024 || a.offset+pos+compressed>a.file.size)throw Error('Invalid MPQ bounds');
          const url=URL.createObjectURL(a.file.slice(a.offset+pos,a.offset+pos+compressed));
          try {
            const xhr=new XMLHttpRequest();xhr.open('GET',url,false);
            xhr.overrideMimeType('text/plain; charset=x-user-defined');xhr.send();
            if(xhr.status!==200)throw Error('Local MPQ read failed');
            const raw=Uint8Array.from(xhr.responseText,c=>c.charCodeAt(0)&255);
            return extract(name,raw,pos,size,flags,a.sector);
          } finally {URL.revokeObjectURL(url);}
        }
        return null;
      },
      async read(name) {
        for (const a of [...archives].reverse()) {
          const b = a.index.lookup(name);
          if (!b.length) continue;
          const [pos, compressed, size, flags] = b;
          if ((flags & 0x2000000) || !(flags & 0x80000000)) return null;
          return extract(name, await slice(a.file, a.offset+pos, compressed), pos, size, flags, a.sector);
        }
        return null;
      },
      async list() {
        const names = new Set();
        for (const a of archives) {
          const b = a.index.lookup('(listfile)');
          if (!b.length) continue;
          const [pos, compressed, size, flags] = b;
          const data = extract('(listfile)', await slice(a.file,a.offset+pos,compressed),pos,size,flags,a.sector);
          for (const n of new TextDecoder().decode(data).split(/[;\r\n]/)) if (n.trim()) names.add(n.trim().replaceAll('/','\\').toLowerCase());
        }
        return [...names].filter(n => {
          for (const a of [...archives].reverse()) { const b=a.index.lookup(n); if (b.length) return !(b[3]&0x2000000) && !!(b[3]&0x80000000); }
          return false;
        });
      },
    };
    const toc = await chain.read('Interface\\FrameXML\\FrameXML.toc');
    if (!toc || !/##\s*Interface:\s*11200/.test(new TextDecoder().decode(toc))) throw Error('This is not a WoW 1.12.1 Data folder (expected build 5875 / interface 11200).');
    return chain;
  } catch(e) { for (const a of archives) a.index.free(); throw e; }
}
