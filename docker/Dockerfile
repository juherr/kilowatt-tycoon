# Container image for the WASM build of Kilowatt Tycoon, served by nginx.
#
# The build stage runs on the build host's native platform: the WASM output is
# architecture-independent, so only the final nginx stage is per-platform.

# Rust 1.94 was stable when the last upstream GitHub Pages build succeeded
# (2026-03-11). Bumping it is a deliberate change, not a side effect.
FROM --platform=$BUILDPLATFORM rust:1.94.0-slim-bookworm AS build

# Same Trunk version as the upstream Pages workflow.
ARG TRUNK_VERSION=v0.21.14

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown

RUN set -eu; \
    asset="trunk-$(uname -m)-unknown-linux-gnu.tar.gz"; \
    base="https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}"; \
    cd /tmp; \
    curl -fsSLO "${base}/${asset}"; \
    curl -fsSL "${base}/${asset}.sha256" | awk '{print $1"  '"${asset}"'"}' | sha256sum -c -; \
    tar -xzf "${asset}" -C /usr/local/bin trunk; \
    rm "${asset}"; \
    trunk --version

WORKDIR /src
COPY . .

# The Supabase leaderboard stays disabled: SUPABASE_URL / SUPABASE_ANON_KEY are
# deliberately not provided. Trunk downloads wasm-bindgen (version from
# Cargo.lock) and wasm-opt at build time. --locked fails the build instead of
# silently re-resolving dependencies when Cargo.lock is out of date.
RUN trunk build --release --locked

FROM nginxinc/nginx-unprivileged:1.31.6-alpine

COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/dist /usr/share/nginx/html

EXPOSE 8080
