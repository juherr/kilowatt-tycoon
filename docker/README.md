# Container image

This fork only adds packaging on top of
[kwtycoon/kilowatt-tycoon](https://github.com/kwtycoon/kilowatt-tycoon): the game
code is unchanged. The WASM build is served by `nginx-unprivileged` (port 8080,
non-root) and published as `ghcr.io/juherr/kilowatt-tycoon:X.Y.Z`
(`linux/amd64` + `linux/arm64`, no `latest`) by `.github/workflows/image.yml` on
every `vX.Y.Z` tag. Pull requests build and smoke-test the image without pushing.

The build uses `trunk build --release --locked` (fails if `Cargo.lock` is stale).
`assets/logokit` (the ~72 MB brand kit, unused at runtime) is excluded through
`.dockerignore`.

The upstream GitHub Pages workflow and `CNAME` are removed so the fork never tries
to publish kwtycoon.com.

## Build and run locally

```bash
docker build -f docker/Dockerfile -t kilowatt-tycoon:local .
docker run --rm -p 8080:8080 kilowatt-tycoon:local
```

Then open <http://127.0.0.1:8080/>. The container is stateless (saves live in the
browser) and runs fine with a read-only root filesystem, given tmpfs mounts on
`/tmp` and `/var/cache/nginx`.

## Smoke test

```bash
docker/smoke-test.sh kilowatt-tycoon:local
```

It runs the image read-only (tmpfs `/tmp` and `/var/cache/nginx`,
`no-new-privileges`) and asserts the nginx contract below plus a non-root user.
CI runs it before any push.

## nginx behaviour

- **No SPA fallback**: `Trunk.toml` sets `no_spa = true` because Bevy probes
  optional `.meta` files and needs a real 404, never `index.html`.
- `/` and `index.html` are served with `Cache-Control: no-cache`; the
  content-hashed `*.wasm` / `*.js` bundle is `immutable` on successful responses
  only (a 404 for a bundle name carries no cache header).
- `*.wasm` is served as `application/wasm`; WASM, JS, CSS, JSON, SVG and plain
  text are gzip-compressed.
- Dotfiles are denied (403), including paths that would also match the bundle
  pattern.

## Streaming OCPP to a CSMS

The WASM build already reads the `ocpp_endpoint` query parameter
(`src/ocpp/mod.rs`); each charger then connects to
`{ocpp_endpoint}/{charger_id}`:

```
http://127.0.0.1:8080/?ocpp_endpoint=wss://csms.example.com/ocpp
```

Without the parameter, no OCPP connection is opened.

## Updating from upstream

```bash
git remote add upstream https://github.com/kwtycoon/kilowatt-tycoon.git  # once
git fetch upstream
git merge upstream/main          # packaging lives in new files: no conflicts expected
git push origin main
git tag vX.Y.Z && git push origin vX.Y.Z
```
