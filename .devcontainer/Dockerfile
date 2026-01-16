FROM mcr.microsoft.com/devcontainers/rust:1-1

RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*
