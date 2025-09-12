# Multi-stage build
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p chatroom

FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/chatroom /usr/local/bin/chatroom
EXPOSE 8080
ENV RUST_LOG=info
CMD ["/usr/local/bin/chatroom"]

