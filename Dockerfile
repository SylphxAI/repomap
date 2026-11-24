# repomap MCP server (stdio). Mount the repository to map at /workspace:
#   docker run -i --rm -v "$PWD:/workspace:ro" ghcr.io/sylphxai/repomap
# Fast catalog build from the published binary. Dockerfile.release still bundles
# the embedding model for the GHCR image.
FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG TARGETARCH
RUN set -eu; \
    case "$TARGETARCH" in \
      amd64) arch=x64; sha=e15c961b869057aae2be3f67f01e368f77111bdf3c41bd63ad51d00aee57d87d ;; \
      arm64) arch=arm64; sha=8b007de036f951b1dd50dd20e76d8a48abdeda95cc5611d3e88ebe5ecff1de39 ;; \
      *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    apt-get update; \
    apt-get install -y --no-install-recommends git ca-certificates curl; \
    rm -rf /var/lib/apt/lists/*; \
    git config --system --add safe.directory '*'; \
    curl -fsSL --retry 3 "https://github.com/SylphxAI/repomap/releases/download/v1.9.0/repomap-linux-${arch}-gnu.tar.gz" -o /tmp/repomap.tar.gz; \
    echo "$sha  /tmp/repomap.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/repomap.tar.gz --no-same-owner -C /usr/local/bin repomap; \
    rm /tmp/repomap.tar.gz
# Keyword search works without any model download, including at startup.
ENV REPOMAP_CACHE_DIR=/tmp/repomap-cache REPOMAP_EMBED=0
WORKDIR /workspace
LABEL org.opencontainers.image.source="https://github.com/SylphxAI/repomap" \
      org.opencontainers.image.description="A map of your codebase for AI agents: code graph, search, call paths and change impact (MCP stdio server)." \
      org.opencontainers.image.licenses="MIT" \
      io.modelcontextprotocol.server.name="io.github.SylphxAI/repomap"
ENTRYPOINT ["repomap", "mcp"]
