# Working in this repository

Azeroll is a Katamari Damacy-style game set in vanilla WoW, built as a server-less mode on the
[benilla](https://github.com/samwhosung/benilla) engine (a from-scratch WoW 1.12.1 client in
Rust + Bevy) by way of its browser fork [wenilla](https://github.com/Arnesen/wenilla). It runs
natively and in the browser (WebGPU), and needs no server, login or account.

Read [docs/azeroll/ARCHITECTURE.md](docs/azeroll/ARCHITECTURE.md) before changing game code: it
holds the numbered decisions (D1, D2, …) the code comments cite, the telemetry and autopilot
levers, and the open work. Engine code follows upstream's map and rules in `docs/METHOD.md`,
`docs/MAP.md` and `docs/CONTRIBUTING.md`.

## What lives where

| path | what |
|---|---|
| `crates/benilla-app/src/azeroll/` | the game: ball, collection, camera, creatures, NPCs, HUD, sound, level |
| `crates/benilla-app/src/azeroll/goldshire.toml`, `goldshire-npcs.toml` | the authored Goldshire level and cast |
| `crates/azeroll` | the native launcher (`cargo azeroll`) |
| `crates/wenilla` | the wasm entry crate; calls `benilla_app::run_web` |
| `crates/wenilla-host` | local dev server: static files, `/data/*` from the MPQs, a `/ws/*` relay Azeroll never opens. Loopback only. |
| `crates/wenilla-mpq-web` | the browser's MPQ reader, used by the service worker for a player's local data folder |
| `crates/benilla-*` | the engine: formats, world, UI, protocol, app |
| `web/` | the page: `index.html`, `boot.js`, the data loaders, `wasi_stubs.js` |
| `assets/music/` | the author's original soundtrack |
| `docs/azeroll/` | design notes, research and data hosting |
| `scripts/web-setup.sh`, `web-build.sh`, `win-build.sh` | wasm toolchain and build; Windows cross-build |
| `.github/workflows/pages.yml` | builds the browser game to GitHub Pages on every push to `master` |

## Commands

```bash
make help                                   # the common targets
WOW_DATA=$PWD/wow-data/Data cargo azeroll   # native, `play` profile
scripts/web-setup.sh                        # once per machine: wasm target, wasm-bindgen-cli at Cargo.lock's version, wasi-sdk, binaryen
scripts/web-build.sh                        # → web/dist. WEB_DEBUG=1 keeps symbols for readable stack traces.
cargo run --release -p wenilla-host -- --www web/dist --data wow-data/Data   # http://127.0.0.1:8090/
cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
cargo test -p benilla-app --lib azeroll     # the game's own tests
```

A browser for testing needs WebGPU; Linux Chrome needs `--enable-unsafe-webgpu`, and the NVIDIA +
Wayland flags are in ARCHITECTURE.md, "Browser build". Without an adapter the client aborts in
`wgpu::create_bind_group_layout` at world entry: that is the missing adapter, not your change.

## Invariants

- **Blizzard data stays out of git.** Game files live in `wow-data/` (ignored) or the R2 bucket
  described in `docs/azeroll/data-hosting.md`.
- **Engine edits are few and generic.** Game logic goes in `azeroll/`; an engine file gets only a
  small seam (the claim seam, `Carried`, the mode hooks), so engine updates stay mergeable.
- **What ships is the `cp` lines in `scripts/web-build.sh`.** A page-side file not on them never
  reaches `web/dist` or Pages.
- **No threads, no std time on wasm.** `std::time::Instant`, `SystemTime` and `env::vars_os`
  panic there; use `bevy::platform::time::Instant` or `web-time`. Check the wasm target after any
  engine change.
- **`wasm-bindgen` crate and CLI versions must match exactly**; `web-setup.sh` enforces it.
- **Pull requests go to `itechify/azeroll`, base `master`.** The local `benilla` and `wenilla`
  remotes are read-only engine sources: pass `-R itechify/azeroll` to `gh pr create`.

## Where to look when

| symptom | look at |
|---|---|
| the wasm panics at boot with `RuntimeError: unreachable` | rebuild with `WEB_DEBUG=1`; check for a WebGPU adapter first; `?bridge=0` rules the bridge out |
| wasm-bindgen glue errors at load | version drift: rerun `scripts/web-setup.sh` |
| a black or frozen canvas in Chromium on Linux | the Vulkan/ANGLE and vsync flags in ARCHITECTURE.md, "Browser build" |
| an unattended native run produces black screenshots | `WOW_NOVSYNC=1` |
| the Pages build cannot find its data | `web/azeroll-config.js` and `docs/azeroll/data-hosting.md` |
