# ----------------------------------------------------
# Stage 1: Build binary using official Rust image
# ----------------------------------------------------
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/findora-backend

# Install build dependencies (OpenSSL / pkg-config)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Copy real application source code
COPY src ./src
COPY migrations ./migrations

# Touch to trigger recompile of main.rs and build release binary
RUN touch src/main.rs && cargo build --release

# ----------------------------------------------------
# Stage 2: Minimal runtime image (~30MB)
# ----------------------------------------------------
FROM debian:bookworm-slim

WORKDIR /app

# Install ca-certificates for secure TLS to Supabase
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from builder
COPY --from=builder /usr/src/findora-backend/target/release/findora-backend /app/findora-backend

ENV PORT=10000
ENV HOST=0.0.0.0

EXPOSE 10000

CMD ["/app/findora-backend"]
