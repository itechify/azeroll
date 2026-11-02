// Browser-local File snapshots and optional directory handles. No archive bytes go over HTTP.
export async function store(key, value) {
  const db = await new Promise((resolve,reject) => {
    const r=indexedDB.open('azeroll-local-data',1);
    r.onupgradeneeded=()=>r.result.createObjectStore('selections');
    r.onsuccess=()=>resolve(r.result); r.onerror=()=>reject(r.error);
  });
  try { return await new Promise((resolve,reject) => {
    const tx=db.transaction('selections', value===undefined?'readonly':'readwrite');
    const r=value===undefined?tx.objectStore('selections').get(key):tx.objectStore('selections').put(value,key);
    tx.oncomplete=()=>resolve(r.result); tx.onerror=()=>reject(tx.error); tx.onabort=()=>reject(tx.error);
  }); } finally { db.close(); }
}
