FROM rust:1.76.0
ARG SQLX_OFFLINE=true
WORKDIR /var/www/microservice/
COPY . .
RUN cargo install --path .
CMD ["rust_microservice"]