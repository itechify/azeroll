// Verify local MPQ extraction after scripts/web-build.sh. No game files are written.
// DATA_FOLDER=/path/to/Data node scripts/mpq-web-smoke.mjs
import fs from 'node:fs';
import {openAsBlob} from 'node:fs';
import init from '../web/dist/wenilla_mpq_web.js';
import {openChain} from '../web/dist/local-mpq.js';
await init({module_or_path:fs.readFileSync('web/dist/wenilla_mpq_web_bg.wasm')});
const folder=process.env.DATA_FOLDER || 'wow-data/Data';
const files=await Promise.all(fs.readdirSync(folder).filter(n=>/\.mpq$/i.test(n)).map(async name=>{const f=await openAsBlob(folder+'/'+name);Object.defineProperty(f,'name',{value:name});return f;}));
const chain=await openChain(files);
const names=[...new Set(['boot','world'].flatMap(n=>JSON.parse(fs.readFileSync(`web/${n}-manifest.json`)).names))];
const list=await chain.list();
for(const ext of ['.m2','.blp','.adt'])names.push(...list.filter(n=>n.endsWith(ext)).slice(0,12));
let missing=[];let count=0;const start=Date.now();
for(const name of names){const bytes=await chain.read(name);if(name==='__index')continue;if(bytes===null)missing.push(name);else count++;}
console.log(JSON.stringify({count,missing,seconds:(Date.now()-start)/1000,index:list.length},null,2));
chain.dispose();
