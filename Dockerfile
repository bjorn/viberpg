FROM rust:1.83 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY data ./data
COPY public ./public
COPY docs ./docs
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/onlinerpg /app/onlinerpg
COPY data ./data
COPY public ./public
COPY docs ./docs

ENV RUST_LOG=info
EXPOSE 3000
CMD ["./onlinerpg"]
