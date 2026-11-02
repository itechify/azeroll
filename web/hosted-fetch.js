// R2's development endpoint throttles asset bursts. Bound simultaneous downloads and retry
// transient network/429/5xx failures; ordinary missing files still return immediately.
export function installHostedFetch(base) {
  const original=window.fetch.bind(window);
  let active=0, nextStart=0;
  const waiting=[];
  const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const acquire=()=>active<8?(active++,Promise.resolve()):new Promise(resolve=>waiting.push(resolve));
  const release=()=>{const next=waiting.shift();if(next)next();else active--;};
  window.fetch=async(input,options)=>{
    const url=typeof input==='string'?input:input.url||String(input);
    if(!url.startsWith(base.replace(/\/$/,'')+'/'))return original(input,options);
    await acquire();
    try {
      const now=performance.now(), start=Math.max(now,nextStart); nextStart=start+25;
      if(start>now)await delay(start-now);
      for(let attempt=0;;attempt++) {
        try {
          const response=await original(input,options);
          if((response.status===429 || response.status>=500) && attempt<3) {
            await response.body?.cancel();await delay(500*2**attempt);continue;
          }
          // Keep the slot until the body finishes; fetch() alone resolves at the headers.
          const bytes=await response.arrayBuffer();
          return new Response(bytes,{status:response.status,statusText:response.statusText,headers:response.headers});
        } catch(error) {
          if(options?.signal?.aborted||error.name==='AbortError'||attempt>=3)throw error;
          await delay(500*2**attempt);
        }
      }
    } finally {release();}
  };
}
