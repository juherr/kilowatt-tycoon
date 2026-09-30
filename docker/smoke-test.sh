#!/usr/bin/env bash
# Smoke test for the container image: nginx routing/caching contract and
# runtime hardening. Runs the image exactly as it is deployed (read-only
# rootfs, tmpfs, no-new-privileges) and asserts over HTTP.
#
#   docker/smoke-test.sh <image>        # e.g. kilowatt-tycoon:local
set -euo pipefail

image=${1:?usage: docker/smoke-test.sh <image>}
port=${SMOKE_PORT:-18080}
name="kwt-smoke-$$"
base="http://127.0.0.1:${port}"
failures=0

cleanup() { docker rm -f "$name" >/dev/null 2>&1 || true; }
trap cleanup EXIT

# The `.hidden` tmpfs lets the test plant a dotfile without a host bind mount
# (the rootfs itself is read-only).
docker run -d --name "$name" -p "127.0.0.1:${port}:8080" \
  --read-only --tmpfs /tmp --tmpfs /var/cache/nginx \
  --mount type=tmpfs,destination=/usr/share/nginx/html/.hidden,tmpfs-mode=1777 \
  --security-opt no-new-privileges:true \
  "$image" >/dev/null

for _ in $(seq 1 30); do
  curl -fso /dev/null "$base/" && break
  sleep 0.5
done

# A hidden file whose name also matches the hashed-bundle regex.
docker exec "$name" sh -c \
  'echo secret > /usr/share/nginx/html/.hidden/leak-0123456789abcdef.js'

pass() { echo "ok   - $1"; }
fail() { echo "FAIL - $1"; failures=$((failures + 1)); }

# check <description> <path> <expected status> [header regex] [curl args...]
# The header regex (case-insensitive) must match; prefix it with `!` to
# require that NO header line matches.
check() {
  local desc=$1 path=$2 want=$3 header=${4:-} headers status
  shift $(( $# < 4 ? $# : 4 ))
  headers=$(curl -s -o /dev/null -D - "$@" "$base$path" | tr -d '\r')
  status=$(printf '%s\n' "$headers" | awk 'NR==1 {print $2}')
  if [ "$status" != "$want" ]; then
    fail "$desc (status $status, want $want)"
    return
  fi
  if [ -n "$header" ]; then
    if [ "${header#!}" != "$header" ]; then
      if printf '%s\n' "$headers" | grep -qiE "${header#!}"; then
        fail "$desc (unexpected header /${header#!}/)"; return
      fi
    elif ! printf '%s\n' "$headers" | grep -qiE "$header"; then
      fail "$desc (missing header /$header/)"; return
    fi
  fi
  pass "$desc"
}

immutable='^cache-control: public, max-age=31536000, immutable$'

wasm=$(curl -fs "$base/" | grep -oE '/[A-Za-z0-9_]+-[0-9a-f]{16}_bg\.wasm' | head -n1)
js=$(curl -fs "$base/" | grep -oE '/[A-Za-z0-9_]+-[0-9a-f]{16}\.js' | head -n1)
[ -n "$wasm" ] && [ -n "$js" ] || fail "index.html references a hashed .wasm and .js bundle"

check "/ is served"                         /             200 '^cache-control: no-cache$'
check "index.html is revalidated"           /index.html   200 '^cache-control: no-cache$'
check "hashed wasm is immutable"            "$wasm"       200 "$immutable"
check "wasm has the right content type"     "$wasm"       200 '^content-type: application/wasm$'
check "wasm is gzip-compressed"             "$wasm"       200 '^content-encoding: gzip$' -H 'Accept-Encoding: gzip'
check "hashed js is immutable"              "$js"         200 "$immutable"
check "missing hashed bundle: 404, not cached" \
      /kilowatt_tycoon-0000000000000000_bg.wasm        404 "!^cache-control:"
check "missing .meta: real 404 (no SPA fallback)" \
      /assets/does-not-exist.png.meta                 404
check "dotfiles are denied"                 /.hidden/plain 403
check "hidden file matching the bundle regex is denied" \
      /.hidden/leak-0123456789abcdef.js               403

uid=$(docker exec "$name" id -u)
if [ "$uid" != 0 ]; then pass "runs as non-root (uid $uid)"; else fail "runs as root"; fi

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
