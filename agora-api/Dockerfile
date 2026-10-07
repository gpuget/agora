FROM rust:1 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p agora-api

FROM debian:stable-slim
RUN useradd --system --no-create-home app
COPY --from=builder /app/target/release/agora-api /usr/local/bin/agora-api
USER app
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["agora-api"]
