FROM rust:1 AS builder
WORKDIR /app
RUN rustup target add wasm32-unknown-unknown && cargo install --locked trunk
COPY . .
WORKDIR /app/frontend
RUN trunk build --release

FROM nginx:alpine
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
EXPOSE 3000
