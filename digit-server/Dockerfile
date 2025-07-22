FROM rust:1.88-slim as builder
WORKDIR /usr/src/digit-server
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /usr/src/digit-server
COPY --from=builder /usr/src/digit-server .
CMD ["./target/release/digit-server"]
