FROM rust:1.88-slim

WORKDIR /usr/src/digit-server
COPY . .

RUN cargo build --release

CMD ["target/release/digit-server"]
