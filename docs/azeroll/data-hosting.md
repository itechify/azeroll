# Game-data hosting (Cloudflare R2)

The GitHub Pages build (https://itechify.github.io/azeroll/) has no data folder to read from, so
the WoW 1.12.1 (build 5875, enUS) files it needs are served as plain static files from a public
Cloudflare R2 bucket.

**Legal note.** The files are Blizzard's content. They are hosted on the project owner's own
Cloudflare R2 account (explicitly approved by them), never in git, and never in the Pages
artifact. The bucket is public: anyone with the URL can download them. Take it down by disabling
the public URL (`PUT .../r2/buckets/azeroll-data/domains/managed {"enabled": false}`) or deleting
the bucket.

## Where

| | |
|---|---|
| bucket | `azeroll-data` (location hint WNAM, Standard) |
| public base URL | `https://pub-864f47fb42254c329900e1e2c75765ba.r2.dev/1.12.1-5875/` |
| object key | `1.12.1-5875/<name>`, `<name>` = the MPQ path lowercased with `\` replaced by `/` |
| example | `Fonts\FRIZQT__.TTF` → `.../1.12.1-5875/fonts/frizqt__.ttf` |
| name list | `.../1.12.1-5875/__index`: the JSON array of original-case MPQ names (what `wenilla-host` serves at `/data/__index`) |

Names with spaces or apostrophes work URL-encoded (`%20`, `%27` or a literal `'`). No name in the
chain contains `#`.

`r2.dev` is Cloudflare's rate-limited, no-SLA development URL. It is fine for this project; if it
is ever throttled, attach a custom domain to the bucket and change only the base URL.

## What is hosted

**Everything the patch chain exposes.** 79,733 objects (79,732 files plus `__index`), 6,780,412,946
bytes (6.78 GB). The set is the union of the
`(listfile)` of the 14 MPQs in `wow-data/Data`, resolved through the same chain the game uses
(`patch-2 > patch > base MPQs`; see `Chain::list` in `crates/benilla-formats/src/chain.rs`). Every
archive's listfile is complete for this install (hash entries = listfile lines + 2), so nothing is
left unnamed. The total is small enough that no exclusions were made: not `speech`, not other
continents, not other maps' terrain.

Exceptions:

- `World\Maps\Kalimdor\Kalimdor_33_49.adt` is **not hosted**: its entry in `patch.MPQ` is
  truncated (zlib "incomplete or truncated stream", also fails in `wenilla-host`, so it fails on
  native too). Its URL returns 404. Nothing in Elwynn or Goldshire needs it.
- Six names in `web/dist/boot-manifest.json` (`Item\TextureComponents\...Leather_A_05Yellow_..._U.blp`)
  are not in the chain at all, so `wenilla-host` 404s them as well, and so does the bucket.

Per object:

- `Cache-Control: public, max-age=31536000, immutable`
- `Content-Type`: `.ttf` → `font/ttf`, `.json` and `__index` → `application/json`,
  `.lua` / `.xml` / `.toc` → `text/plain`, anything else `application/octet-stream`.

Bucket CORS: GET and HEAD from `https://itechify.github.io` and from `localhost` / `127.0.0.1` on
ports 3000, 4173, 5173, 8000, 8080, 8090, 8099 and 8197; allowed header `Range`; exposed
`Content-Length`, `Content-Range`, `Content-Type`, `ETag`; max-age 86400. R2 rejects wildcard
ports (`http://localhost:*` is MalformedXML), so the dev ports are listed in
`CORS_ORIGINS` in `scripts/r2-upload.py`; add a port there and run `configure` again to serve from
another one. Other origins get no `Access-Control-Allow-Origin`.

The page is cross-origin isolated (COEP `require-corp`). A CORS-mode `fetch()` of these objects
passes that check without a `Cross-Origin-Resource-Policy` header, so the client must fetch them
with `mode: 'cors'` (the default for `fetch`), not as `no-cors` subresources.

## Free-tier headroom (R2 Standard, per month)

| | used | free tier | left |
|---|---|---|---|
| storage | 6.78 GB | 10 GB | 3.2 GB |
| Class A (writes, listing) | about 80,000 for the full upload, plus about 200 listing calls per re-run | 1,000,000 | a full re-upload costs 8% of a month |
| Class B (reads) | about 100,000 from verification and uploads | 10,000,000 | a cold load reads thousands of objects |
| egress | 0 | free | R2 does not charge egress |

Class B is what a popular page would spend: 10M reads is roughly a few thousand cold loads per month.
Browsers cache the immutable objects after the first visit.

## Re-running it

`scripts/r2-upload.py` does everything. It needs python3 and boto3 (nothing else is installed):

```bash
python3 -m venv ~/azeroll-r2-tools/venv && ~/azeroll-r2-tools/venv/bin/pip install boto3
```

Credentials come from the environment or from files named by `<VAR>_FILE`; the script never prints
or stores them:

| variable | what |
|---|---|
| `CF_API_TOKEN` | Cloudflare API token, R2 Admin Read & Write (bucket create, managed public URL; also finds the account id) |
| `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` | R2 S3 API token pair (CORS, upload) |
| `R2_ACCOUNT_ID` | optional when `CF_API_TOKEN` is set |

```bash
R="python3 scripts/r2-upload.py"   # or the venv's python
$R configure      # idempotent: bucket, CORS, managed r2.dev URL; prints the base URL

# a running wenilla-host is the extractor, so the patch chain is resolved exactly as the game does:
target/release/wenilla-host --bind 127.0.0.1:8197 --www web/dist --data wow-data/Data &
$R extract --host http://127.0.0.1:8197 --out ~/azeroll-r2-staging     # ~6.8 GB; NOT /tmp, NOT in the repo
$R upload  --src ~/azeroll-r2-staging                                   # parallel, resumable
$R verify  --base https://pub-….r2.dev/1.12.1-5875/ --host http://127.0.0.1:8197 \
           --manifest web/dist/boot-manifest.json                       # byte-for-byte vs the host
rm -rf ~/azeroll-r2-staging
```

- `upload` skips every key already in the bucket with the same size, so an interrupted run resumes.
  The names are lowercased, so the staging dir needs a case-sensitive filesystem (any Linux one).
- Short on disk (the staging dir needs the full 6.8 GB): alternate `extract --skip-remote` and
  `upload --delete-after`. The first leaves out names already in the bucket, the second removes
  each staged file once the bucket confirms its size.
- `extract` refuses a staging directory inside the repo and fails on two names that collide after
  lowercasing (none do).
- `verify` fetches each name from the host and from the bucket, with and without
  `Origin: https://itechify.github.io`, and compares bytes, `Content-Type`, `Cache-Control` and
  `Access-Control-Allow-Origin`. It sends its own `User-Agent`: Cloudflare answers 403 to the
  default `Python-urllib` one.
- Re-run it for a different client build by changing `PREFIX` (the `1.12.1-5875` segment is the
  build, so old and new data can sit side by side).

## Verification record (2 Nov 2026)

- Bucket listing matches the extraction: 79,733 objects, every size equal to the staged file's.
- `verify`: all 403 `boot-manifest.json` names plus 30 random and special names, then another 360
  (seed 7; includes names with spaces and apostrophes): bytes identical to `wenilla-host`,
  `Content-Type` and `Cache-Control` as above, `Access-Control-Allow-Origin` echoing the page origin;
  0 mismatches. The 6 manifest names that the chain lacks 404 on both sides.
- Checked by hand with `curl`: `OPTIONS` preflight with `Access-Control-Request-Headers: range` → 204
  with `Allow-Methods: GET, HEAD`, `Allow-Headers: range`, `Max-Age: 86400`; a `Range: bytes=0-15`
  request → `206` with `Content-Range`; a foreign origin gets no `Access-Control-Allow-Origin`.
