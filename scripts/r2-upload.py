#!/usr/bin/env python3
"""Host Azeroll's WoW 1.12.1 game data on a Cloudflare R2 bucket.

See docs/azeroll/data-hosting.md. Four steps, each safe to re-run:

  configure  create the bucket, set its CORS rules, switch on the managed r2.dev URL
  extract    pull every file out of the MPQ patch chain (through a running wenilla-host)
             into a staging directory, as lowercase forward-slash paths
  upload     push the staging directory to the bucket (parallel, resumable: a key that is
             already there with the same size is skipped)
  verify     compare public URLs byte for byte against the local wenilla-host

Credentials come from the environment, or from files named by <VAR>_FILE; nothing is ever
printed. R2_ACCOUNT_ID may be omitted when CF_API_TOKEN is set (looked up from the API).

  CF_API_TOKEN          Cloudflare API token, R2 Admin Read & Write   (configure; account lookup)
  R2_ACCESS_KEY_ID      R2 S3 access key id                           (configure CORS, upload, verify)
  R2_SECRET_ACCESS_KEY  R2 S3 secret access key
  R2_ACCOUNT_ID         Cloudflare account id

Dependencies: python3 and boto3 (`python3 -m venv V && V/bin/pip install boto3`).
"""

import argparse
import concurrent.futures as cf
import json
import os
import random
import shutil
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

BUCKET = "azeroll-data"
PREFIX = "1.12.1-5875/"
CACHE_CONTROL = "public, max-age=31536000, immutable"

# The origins the browser build is served from. R2's CORS rejects a wildcard port
# (`http://localhost:*` is MalformedXML), so the local dev ports are listed out.
DEV_PORTS = (3000, 4173, 5173, 8000, 8080, 8090, 8099, 8197)
CORS_ORIGINS = ["https://itechify.github.io"] + [
    f"http://{h}:{p}" for p in DEV_PORTS for h in ("localhost", "127.0.0.1")
]

CONTENT_TYPES = {
    "ttf": "font/ttf",
    "json": "application/json",
    "lua": "text/plain",
    "xml": "text/plain",
    "toc": "text/plain",
}


def secret(var):
    """An env var, or the contents of the file named by <var>_FILE."""
    v = os.environ.get(var)
    if v:
        return v.strip()
    f = os.environ.get(var + "_FILE")
    if f:
        with open(f) as fh:
            return fh.read().strip()
    return None


def need(var):
    v = secret(var)
    if not v:
        sys.exit(f"missing credential: set {var} or {var}_FILE")
    return v


def account_id():
    acct = secret("R2_ACCOUNT_ID")
    if acct:
        return acct
    res = cf_api("GET", "/accounts")["result"]
    if len(res) != 1:
        sys.exit("CF_API_TOKEN sees several accounts; set R2_ACCOUNT_ID")
    return res[0]["id"]


def cf_api(method, path, body=None):
    req = urllib.request.Request(
        "https://api.cloudflare.com/client/v4" + path,
        method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={
            "Authorization": "Bearer " + need("CF_API_TOKEN"),
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        return json.loads(e.read() or b"{}") | {"http_status": e.code}


def s3_client():
    import boto3
    from botocore.config import Config

    return boto3.client(
        "s3",
        endpoint_url=f"https://{account_id()}.r2.cloudflarestorage.com",
        aws_access_key_id=need("R2_ACCESS_KEY_ID"),
        aws_secret_access_key=need("R2_SECRET_ACCESS_KEY"),
        region_name="auto",
        config=Config(
            signature_version="s3v4",
            retries={"max_attempts": 10, "mode": "adaptive"},
            max_pool_connections=64,
        ),
    )


def key_for(name):
    """MPQ path -> object key tail: lowercased, `\\` as `/`."""
    return name.replace("\\", "/").lower()


def content_type(key):
    if key == "__index":  # the name list wenilla-host serves at /data/__index
        return "application/json"
    return CONTENT_TYPES.get(key.rsplit(".", 1)[-1], "application/octet-stream")


# ---------------------------------------------------------------- configure


def cmd_configure(_):
    acct = account_id()
    r = cf_api("POST", f"/accounts/{acct}/r2/buckets", {"name": BUCKET})
    if r.get("success"):
        print("bucket created")
    elif any(e.get("code") == 10004 for e in r.get("errors", [])):
        print("bucket exists")
    else:
        sys.exit(f"bucket create failed: {r.get('errors')}")
    s3_client().put_bucket_cors(
        Bucket=BUCKET,
        CORSConfiguration={
            "CORSRules": [
                {
                    "AllowedOrigins": CORS_ORIGINS,
                    "AllowedMethods": ["GET", "HEAD"],
                    "AllowedHeaders": ["Range"],
                    "ExposeHeaders": ["Content-Length", "Content-Range", "Content-Type", "ETag"],
                    "MaxAgeSeconds": 86400,
                }
            ]
        },
    )
    print("cors set:", len(CORS_ORIGINS), "origins")
    r = cf_api("PUT", f"/accounts/{acct}/r2/buckets/{BUCKET}/domains/managed", {"enabled": True})
    if not r.get("success"):
        sys.exit(f"enabling r2.dev failed: {r.get('errors')}")
    base = f"https://{r['result']['domain']}/{PREFIX}"
    print("public base:", base)


# ------------------------------------------------------------------ extract


def http_get(url, tries=5):
    for i in range(tries):
        try:
            with urllib.request.urlopen(url, timeout=120) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code == 404:
                raise
            err = e
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
            err = e
        time.sleep(1 + i)
    raise err


def data_url(host, name):
    return f"{host.rstrip('/')}/data/{urllib.parse.quote(name, safe='')}"


def cmd_extract(a):
    out = os.path.abspath(a.out)
    if os.path.commonpath([out, os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))]) == \
            os.path.abspath(os.path.join(os.path.dirname(__file__), "..")):
        sys.exit("refusing to stage game data inside the git repo")
    os.makedirs(out, exist_ok=True)
    index = http_get(f"{a.host.rstrip('/')}/data/__index")
    names = json.loads(index)
    keys = {}
    for n in names:
        k = key_for(n)
        if k in keys:
            sys.exit(f"two names collide after lowercasing: {n!r} / {keys[k]!r}")
        keys[k] = n
    with open(os.path.join(out, "__index"), "wb") as f:
        f.write(index)
    print(f"{len(names)} names in the chain")
    if a.skip_remote:
        # Low-disk mode: extract only what the bucket does not hold yet, so `upload
        # --delete-after` can empty the staging dir between rounds.
        have = remote_sizes(s3_client())
        keys = {k: n for k, n in keys.items() if k not in have}
        print(f"{len(keys)} not yet in the bucket")

    done = failed = 0
    nbytes = 0
    lock = threading.Lock()
    t0 = time.time()

    def fetch(item):
        k, n = item
        path = os.path.join(out, *k.split("/"))
        if os.path.exists(path):
            return os.path.getsize(path)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        body = http_get(data_url(a.host, n))
        tmp = path + ".part"
        with open(tmp, "wb") as f:
            f.write(body)
        os.replace(tmp, path)
        return len(body)

    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(fetch, it): it for it in keys.items()}
        for fu in cf.as_completed(futs):
            try:
                sz = fu.result()
            except Exception as e:  # noqa: BLE001
                failed += 1
                print("FAILED", futs[fu][1], e, file=sys.stderr)
                continue
            with lock:
                done += 1
                nbytes += sz
                if done % 5000 == 0:
                    print(f"  {done}/{len(keys)}  {nbytes/1e9:.2f} GB  {time.time()-t0:.0f}s", flush=True)
    print(f"extracted {done} files, {nbytes/1e9:.3f} GB, {failed} failed")
    sys.exit(1 if failed else 0)


# ------------------------------------------------------------------- upload


def walk(root):
    for d, _, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            rel = os.path.relpath(p, root).replace(os.sep, "/")
            yield rel, p


def remote_sizes(s3):
    out = {}
    for page in s3.get_paginator("list_objects_v2").paginate(Bucket=BUCKET, Prefix=PREFIX):
        for o in page.get("Contents", []):
            out[o["Key"][len(PREFIX):]] = o["Size"]
    return out


def cmd_upload(a):
    s3 = s3_client()
    local = {rel: p for rel, p in walk(a.src) if not rel.endswith(".part")}
    print(f"{len(local)} local files")
    remote = remote_sizes(s3)
    expected = {k: os.path.getsize(p) for k, p in local.items()}
    # `__index` is tiny and its content type is easy to get wrong, so it is always re-put.
    todo = [(k, p) for k, p in local.items() if remote.get(k) != expected[k] or k == "__index"]
    print(f"{len(remote)} already in the bucket; {len(todo)} to upload")
    done = failed = 0
    nbytes = 0
    lock = threading.Lock()
    t0 = time.time()

    def put(item):
        k, p = item
        extra = {"ContentType": content_type(k), "CacheControl": CACHE_CONTROL}
        size = os.path.getsize(p)
        s3.upload_file(p, BUCKET, PREFIX + k, ExtraArgs=extra)
        if a.delete_after:
            # Only after the bucket confirms the size.
            if s3.head_object(Bucket=BUCKET, Key=PREFIX + k)["ContentLength"] == size:
                os.remove(p)
        return size

    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(put, it): it for it in todo}
        for fu in cf.as_completed(futs):
            try:
                sz = fu.result()
            except Exception as e:  # noqa: BLE001
                failed += 1
                print("FAILED", futs[fu][0], e, file=sys.stderr)
                continue
            with lock:
                done += 1
                nbytes += sz
                if done % 2000 == 0:
                    print(f"  {done}/{len(todo)}  {nbytes/1e9:.2f} GB  {time.time()-t0:.0f}s", flush=True)
    print(f"uploaded {done} objects, {nbytes/1e9:.3f} GB, {failed} failed")
    if failed:
        sys.exit(1)
    # Final accounting straight from the bucket.
    final = remote_sizes(s3)
    total = sum(final.values())
    print(f"bucket now holds {len(final)} objects, {total} bytes ({total/1e9:.3f} GB) under {PREFIX}")
    wrong = [k for k, sz in expected.items() if final.get(k) != sz]
    if wrong:
        sys.exit(f"{len(wrong)} local files missing or wrong-sized in the bucket, e.g. {wrong[:3]}")
    print("every local file is present in the bucket with the same size")


# ------------------------------------------------------------------- verify


def cmd_verify(a):
    base = a.base.rstrip("/") + "/"
    names = json.loads(http_get(f"{a.host.rstrip('/')}/data/__index"))
    manifest = json.load(open(a.manifest))["names"] if a.manifest else []
    manifest = [n for n in manifest if n != "__index"]
    rnd = random.Random(a.seed)
    sample = rnd.sample(names, min(a.random, len(names)))
    special = [n for n in names if any(c in n for c in " '#")]
    sample += rnd.sample(special, min(a.special, len(special)))
    todo = list(dict.fromkeys(manifest + sample))
    bad = 0

    def pub_url(n):
        return base + urllib.parse.quote(key_for(n), safe="/")

    def fetch(n, origin):
        req = urllib.request.Request(pub_url(n), headers={"User-Agent": "azeroll-r2-verify/1", **({"Origin": origin} if origin else {})})
        with urllib.request.urlopen(req, timeout=120) as r:
            return r.status, dict(r.headers), r.read()

    def check(n):
        problems = []
        try:
            want = http_get(data_url(a.host, n))
        except urllib.error.HTTPError as e:
            # Not readable from the chain (absent, or a damaged archive entry): the bucket must not
            # have it either.
            try:
                fetch(n, None)
                problems.append(f"host says {e.code} but the bucket serves it")
            except urllib.error.HTTPError as pe:
                if pe.code != 404:
                    problems.append(f"host {e.code}, bucket {pe.code}")
                else:
                    print(f"absent on both: {n} (host {e.code})")
            return n, problems
        for origin in (None, "https://itechify.github.io"):
            try:
                st, h, body = fetch(n, origin)
            except Exception as e:  # noqa: BLE001
                problems.append(f"{origin}: {e}")
                continue
            h = {k.lower(): v for k, v in h.items()}
            if body != want:
                problems.append(f"{origin}: bytes differ ({len(body)} vs {len(want)})")
            if h.get("content-type") != content_type(key_for(n)):
                problems.append(f"content-type {h.get('content-type')}")
            if h.get("cache-control") != CACHE_CONTROL:
                problems.append(f"cache-control {h.get('cache-control')}")
            if origin and h.get("access-control-allow-origin") != origin:
                problems.append(f"acao {h.get('access-control-allow-origin')}")
        return n, problems

    with cf.ThreadPoolExecutor(a.jobs) as ex:
        for n, problems in ex.map(check, todo):
            if problems:
                bad += 1
                print("BAD", n, problems)
    print(f"checked {len(todo)} files ({len(manifest)} manifest, {len(sample)} sampled), {bad} bad")
    sys.exit(1 if bad else 0)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("configure").set_defaults(fn=cmd_configure)
    e = sub.add_parser("extract")
    e.add_argument("--host", default="http://127.0.0.1:8090", help="running wenilla-host")
    e.add_argument("--out", required=True, help="staging dir (outside the repo, not /tmp)")
    e.add_argument("--jobs", type=int, default=8)
    e.add_argument("--skip-remote", action="store_true", help="skip names already in the bucket (low-disk rounds)")
    e.set_defaults(fn=cmd_extract)
    u = sub.add_parser("upload")
    u.add_argument("--src", required=True, help="staging dir written by `extract`")
    u.add_argument("--jobs", type=int, default=32)
    u.add_argument("--delete-after", action="store_true", help="remove each staged file once the bucket confirms its size")
    u.set_defaults(fn=cmd_upload)
    v = sub.add_parser("verify")
    v.add_argument("--base", required=True, help="public base URL ending in /1.12.1-5875/")
    v.add_argument("--host", default="http://127.0.0.1:8090")
    v.add_argument("--manifest", help="web/dist/boot-manifest.json")
    v.add_argument("--random", type=int, default=20)
    v.add_argument("--special", type=int, default=10, help="extra names with space/apostrophe/#")
    v.add_argument("--seed", type=int, default=1)
    v.add_argument("--jobs", type=int, default=8)
    v.set_defaults(fn=cmd_verify)
    a = p.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
