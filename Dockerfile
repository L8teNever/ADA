# ---- Build: Rust-Backend ----
FROM rust:1-bookworm AS build
WORKDIR /src
# Abhängigkeiten zuerst (Docker-Cache)
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --locked && rm -rf src
COPY backend/src ./src
RUN touch src/main.rs && cargo build --release --locked

# ---- Runtime ----
FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home /data ada && mkdir -p /data /app && chown ada /data
COPY --from=build /src/target/release/ada /app/ada
COPY web /app/web
ENV PORT=8080 DATA_DIR=/data WEB_DIR=/app/web
USER ada
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s CMD ["/app/ada", "healthcheck"]
ENTRYPOINT ["/app/ada"]
