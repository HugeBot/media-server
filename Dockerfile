# syntax=docker/dockerfile:1

########################################
# Builder: static musl binary
########################################
FROM docker.io/library/rust:1-alpine AS builder

RUN apk add --no-cache musl-dev ca-certificates \
    && addgroup -g 10001 app \
    && adduser -D -u 10001 -G app app \
    && mkdir -p /data && chown -R app:app /data

WORKDIR /build

# Build dependencies first so they're cached across source changes
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

########################################
# Runtime: scratch
########################################
FROM scratch AS runtime

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
COPY --from=builder --chown=app:app /data /data
COPY --from=builder /build/target/release/media-server /usr/local/bin/media-server
COPY buckets.toml /etc/media-server/buckets.toml

USER app:app

ENV BIND_ADDR=0.0.0.0:3000 \
    STORAGE_DIR=/data \
    BUCKETS_CONFIG_PATH=/etc/media-server/buckets.toml

EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/media-server"]
