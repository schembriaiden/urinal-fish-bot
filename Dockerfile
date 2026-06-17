FROM rust:1.96-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/urinal-fish-bot /usr/local/bin/urinal-fish-bot

ENV DATABASE_PATH=/data/bot.db
VOLUME ["/data"]

CMD ["urinal-fish-bot"]
