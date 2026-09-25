# repomap MCP server (stdio). Mount the repository to map at /workspace:
#   docker run -i --rm -v "$PWD:/workspace:ro" ghcr.io/sylphxai/repomap
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p repomap && cp target/release/repomap /repomap

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && git config --system --add safe.directory '*'
COPY --from=build /repomap /usr/local/bin/repomap
ENV REPOMAP_CACHE_DIR=/tmp/repomap-cache SYLPHX_MODEL_DIR=/opt/repomap/models
# The embedding model ships in the image, so the server needs no network.
RUN repomap model
WORKDIR /workspace
LABEL org.opencontainers.image.source="https://github.com/SylphxAI/repomap" \
      org.opencontainers.image.description="A map of your codebase for AI agents: code graph, search, call paths and change impact (MCP stdio server)." \
      org.opencontainers.image.licenses="MIT" \
      io.modelcontextprotocol.server.name="io.github.SylphxAI/repomap"
ENTRYPOINT ["repomap", "mcp"]
