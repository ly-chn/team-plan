# ── Build stage ──
FROM rust:1.87-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
# Create dummy main.rs to cache dependency build
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/db.rs && echo '' > src/holiday.rs
RUN cargo build --release 2>/dev/null || true

# Copy real source and build
COPY src/ src/
# Touch to force rebuild of our code (deps already cached)
RUN touch src/main.rs && cargo build --release

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary and static files
COPY --from=builder /app/target/release/hermes-mimo .
COPY static/ static/

# Create data directory for SQLite
RUN mkdir -p /data && ln -s /data/mimo.db mimo.db

EXPOSE 8080

ENV RUST_LOG=info

VOLUME ["/data"]

CMD ["./hermes-mimo"]
