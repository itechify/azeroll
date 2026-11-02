import test from 'node:test';
import assert from 'node:assert/strict';
import {installHostedFetch} from '../hosted-fetch.js';
test('hosted downloads are bounded, retry network faults and preserve missing-file responses',async()=>{
  let active=0,peak=0,attempts=0;
  globalThis.window={fetch:async(url)=>{
    if(url.endsWith('retry') && attempts++===0)throw new TypeError('network error');
    active++;peak=Math.max(peak,active);
    await new Promise(r=>setTimeout(r,250));active--;
    return new Response('bytes',{status:url.endsWith('missing')?404:200});
  }};
  try {
    installHostedFetch('https://example.com/data/');
    const responses=await Promise.all(Array.from({length:12},(_,i)=>window.fetch('https://example.com/data/'+i)));
    assert.ok(peak<=8);assert.equal(responses.length,12);
    assert.equal(await (await window.fetch('https://example.com/data/retry')).text(),'bytes');
    assert.equal(attempts,2);
    assert.equal((await window.fetch('https://example.com/data/missing')).status,404);
  } finally {delete globalThis.window;}
});
