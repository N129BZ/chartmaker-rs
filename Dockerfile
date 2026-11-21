# Multi-stage / Deployment Dockerfile for chartmaker-rs
FROM ghcr.io/osgeo/gdal:ubuntu-small-latest

LABEL org.opencontainers.image.source=https://github.com

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    unzip \
    zip \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /chartmaker

# Copy runtime assets and the compiled binary
COPY target/release/chartmaker /chartmaker/chartmaker
COPY config.yml /chartmaker/config.yml
COPY chartdates.json /chartmaker/chartdates.json
COPY clipshapes/ /chartmaker/clipshapes/
COPY dist/ /chartmaker/dist/

# Create work, cache, and output folders
RUN mkdir -p /chartmaker/chartcache /chartmaker/mbtiles /chartmaker/workarea \
    && chmod -R 777 /chartmaker

ENV WEBSERVER=true
ENV DOCKER=true

EXPOSE 8500

VOLUME ["/chartmaker/mbtiles", "/chartmaker/chartcache"]

CMD ["/chartmaker/chartmaker"]
