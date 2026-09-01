FROM rust:1.85-bookworm AS build
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates/open-sdbl-cli ./crates/open-sdbl-cli
COPY crates/open-sdbl-trino ./crates/open-sdbl-trino
RUN cargo build --locked --release --package open-sdbl-trino

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /source/target/release/open-sdbl-trino /usr/local/bin/open-sdbl-trino
USER 65532:65532
EXPOSE 8088
ENTRYPOINT ["/usr/local/bin/open-sdbl-trino"]
