FROM rust:1.75

WORKDIR /usr/src/plonky2-playground
COPY . .
RUN cargo test --release --no-run

ENTRYPOINT ["cargo", "test", "--release"]
