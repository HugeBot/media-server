# media-server

A small, fast static media service written in Rust with [Axum](https://github.com/tokio-rs/axum). It accepts image uploads, normalizes them to lossless WebP, stores them per bucket, and serves them back with HTTP caching, conditional requests and `Range` support.

## Features

- **Multipart upload** with per-bucket storage and configurable resizing.
- **Lossless WebP** re-encoding via the [`image`](https://crates.io/crates/image) crate (no extra native dependencies).
- **Configurable buckets**: each bucket has its own max image dimension and optional retention period.
- **Token-protected** upload/delete endpoints (Bearer token).
- **Background cleanup** task that removes expired files per bucket.
- **Streaming file serving** via `tower-http`'s `ServeFile` (conditional `GET`/304, `Range` requests, immutable cache headers).
- Structured request tracing via `tracing` + `tower-http::TraceLayer`.
- Minimal `scratch`-based Docker image and Podman Quadlet units for deployment.

## API

All endpoints are relative to `BIND_ADDR` (or `PUBLIC_BASE_URL` for the public-facing serving URL).

### `GET /health`

Health check. Returns `200 OK` with body `OK`. No authentication required.

### `POST /upload`

Protected (`Authorization: Bearer <API_TOKEN>`). Accepts a `multipart/form-data` body with the following fields:

| Field | Required | Description |
|---|---|---|
| `bucket` | yes | Name of the target bucket, must exist in `buckets.toml`. |
| `image` | yes | The image file (jpeg, png, gif or webp). |
| `max_dimension_override` | no | Resize the longest side to this value instead of the bucket's configured `max_dimension`. Must be between 16 and 4096, and is capped at the bucket's configured `max_dimension` (it can only make images smaller, never larger). |

The image is decoded, resized so its longest side does not exceed the effective max dimension (aspect ratio preserved, never upscaled), re-encoded as lossless WebP, and stored as `{STORAGE_DIR}/{bucket}/{uuid}.webp` (UUIDv7).

Response:

```json
{
  "bucket": "giveaways",
  "image_id": "019eb785-4a32-7f83-b08b-d6fa84cf86c9",
  "url": "https://static-media.huge.bot/giveaways/019eb785-4a32-7f83-b08b-d6fa84cf86c9"
}
```

Example:

```bash
curl -X POST https://static-media.huge.bot/upload \
  -H "Authorization: Bearer $API_TOKEN" \
  -F "bucket=giveaways" \
  -F "image=@photo.jpg" \
  -F "max_dimension_override=512"
```

### `GET /{bucket}/{image_id}`

Public. Streams the stored WebP file. Supports `If-None-Match`/`If-Modified-Since` (returns `304`), `Range` requests, and sets `Cache-Control: public, max-age=31536000, immutable`.

### `DELETE /{bucket}/{image_id}`

Protected (`Authorization: Bearer <API_TOKEN>`). Removes the stored file. Returns `204 No Content`.

## Configuration

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `BIND_ADDR` | `0.0.0.0:3000` | Address the server listens on. |
| `STORAGE_DIR` | `./storage` | Root directory for stored images, one subdirectory per bucket. |
| `PUBLIC_BASE_URL` | `https://static-media.huge.bot` | Base URL used to build the `url` field in upload responses. |
| `MAX_UPLOAD_BYTES` | `26214400` (25 MiB) | Maximum accepted request body size. |
| `API_TOKEN` | *(required)* | Bearer token required for `/upload` and `DELETE`. |
| `BUCKETS_CONFIG_PATH` | `./buckets.toml` | Path to the bucket configuration file (see below). |
| `CLEANUP_INTERVAL_SECS` | `3600` | How often the background cleanup task runs. |
| `RUST_LOG` | `info` | Standard `tracing`/`env_logger`-style filter. |

### `buckets.toml`

Buckets are defined in a TOML file (path set via `BUCKETS_CONFIG_PATH`). Each bucket has:

- `name`: lowercase letters, digits and hyphens only (used as the storage subdirectory and the `{bucket}` path segment). Must be unique.
- `max_dimension`: maximum size in pixels for the longest side after resizing (16–4096).
- `max_age_days` *(optional)*: how many days a file lives before the cleanup task removes it. **Omit this field to make the bucket permanent** (its files are never removed by cleanup).

```toml
[[bucket]]
name = "giveaways"
max_dimension = 1000
max_age_days = 15

[[bucket]]
name = "stream-previews"
max_dimension = 1000
max_age_days = 15

# Permanent bucket: no max_age_days, cleanup never removes its files.
# [[bucket]]
# name = "permanent-assets"
# max_dimension = 2000
```

The server validates this file on startup and panics with a descriptive error if it is missing, empty, or contains an invalid bucket (bad name format, duplicate name, out-of-range `max_dimension`, or `max_age_days = 0`). The configured storage directory for each bucket is created automatically on startup if it doesn't exist.

## Running locally

```bash
API_TOKEN=changeme cargo run
```

This uses `./storage` as the storage directory and `./buckets.toml` for bucket configuration.

## Running with Docker / Podman

A multistage `Dockerfile` builds a static musl binary and produces a minimal `scratch`-based image running as a non-root user, with the default `buckets.toml` baked in at `/etc/media-server/buckets.toml`.

```bash
docker build -t media-server .
docker run --rm -p 3000:3000 \
  -e API_TOKEN=changeme \
  -v media-data:/data \
  media-server
```

### Podman Quadlet

The `quadlet/` directory contains ready-to-use Quadlet units:

- `media-server.container` — runs the image, mounts a named volume at `/data`, and reads `API_TOKEN` from a Podman secret:

  ```bash
  podman secret create media-server-api-token -
  ```

- `media-server-data.volume` — the named volume backing `/data`.

To override the bundled `buckets.toml` without rebuilding the image, uncomment the relevant `Volume=` line in `media-server.container` and bind-mount your own file at `/etc/media-server/buckets.toml`.

Copy both files to `~/.config/containers/systemd/` (or `/etc/containers/systemd/` for system-wide units), then:

```bash
systemctl --user daemon-reload
systemctl --user start media-server.service
```

## CI/CD

On every push to `master`, GitHub Actions builds the Docker image, pushes it to `ghcr.io/hugebot/media-server` tagged `latest` and with the short commit SHA, and signs the resulting image digest with [cosign](https://github.com/sigstore/cosign).
