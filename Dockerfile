# Planner stage: generate dependency recipe (uses cargo-chef)
FROM rust:1.90 AS planner
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates pkg-config build-essential curl && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

WORKDIR /app
# Copy minimal files to create dependency recipe
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() { println!("noop"); }' > src/main.rs
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage: install build deps, sccache and compile using cached deps
FROM rust:1.90 AS builder
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates pkg-config build-essential curl libssl-dev git && rm -rf /var/lib/apt/lists/*

# Install sccache to speed repeated builds (uses cargo install here)
RUN cargo install sccache --locked
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_CACHE_SIZE=10G

# Ensure cargo-chef is available in builder
RUN cargo install cargo-chef --locked

WORKDIR /app
# Copy the prepared recipe and cook dependencies (this layer is cache-friendly)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the full source and build the release binary
COPY . .
RUN cargo build --release --bin wibble

# Runtime image: small, contains only what's needed to run the binary and prisma CLI
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates nodejs npm && rm -rf /var/lib/apt/lists/*

# Install Prisma CLI required by the entrypoint to apply migrations
RUN npm install -g prisma

# Create an unprivileged user to run the app
RUN useradd --no-log-init -m appuser
WORKDIR /app

# Copy the built binary and prisma schema/migrations for runtime migration step
RUN mkdir -p target/release
COPY --from=builder /app/target/release/wibble target/release/wibble
COPY database database
COPY docker-entrypoint.sh .

RUN chmod +x ./docker-entrypoint.sh ./target/release/wibble
USER appuser
ENV PATH="/usr/local/bin:$PATH"

ENTRYPOINT ["/app/docker-entrypoint.sh"]
