# Chartmaker-RS

A high-performance Rust port of **Chartmaker**, an FAA aeronautical raster chart generation system.

Chartmaker-RS downloads digital GeoTIFF aeronautical charts from the FAA, clips them using shapefile cutlines, reprojects them into Web Mercator (`EPSG:3857`), merges multi-tile regions, and creates optimized SQLite `.mbtiles` raster pyramids with Lanczos overviews.

---

## Features

- **Blazing Fast & Low Memory**: Pure native binary (~6.5MB) using <25MB RAM under load (down from ~200MB+ in Node.js).
- **Tokio Subprocess Model**: Non-blocking GDAL child process orchestration (`gdalwarp`, `gdal_translate`, `gdalbuildvrt`, `gdaladdo`) with clean cancellation tokens.
- **Embedded SQLite Tagging**: Updates MBTILES metadata and attribution directly via in-process `rusqlite` without shell-outs.
- **Dual Operating Modes**:
  - **Terminal CLI Mode**: Interactive terminal loop with dynamic menus and "Start over" navigation.
  - **Web Server Mode**: High-performance Axum web server and WebSocket hub streaming real-time job progress to the Vite web frontend.

---

## Prerequisites

- **Rust & Cargo**: Rust 1.80+ (`rustup`)
- **GDAL Command Line Tools**: `gdalwarp`, `gdal_translate`, `gdalbuildvrt`, `gdaladdo`, `gdalinfo`
- **System Utilities**: `curl`, `unzip`, `zip`
- **Node.js & npm** (optional, only needed if rebuilding the frontend in `frontend/`)

---

## Quick Start

### 1. Build

```bash
cargo build --release
```

### 2. Run in Interactive Terminal CLI Mode

```bash
WEBSERVER=false ./target/release/chartmaker
# or
WEBSERVER=false cargo run --release
```

### 3. Run in Web Server Mode

```bash
WEBSERVER=true ./target/release/chartmaker
# or
WEBSERVER=true cargo run --release
```

Once running, navigate to [http://localhost:8500](http://localhost:8500).

---

## Running Tests

```bash
cargo test
```

---

## License

Apache-2.0
