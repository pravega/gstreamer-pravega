#!/bin/bash
set -ex

# Make sure to always have fresh base image
docker pull ubuntu:20.10
# Install dev dependencies
docker build -t pravega/gstreamer:dev-dependencies -f Dockerfile-dev-dependencies .
# Download source code
docker build -t pravega/gstreamer:dev-downloaded -f Dockerfile-dev-downloaded .
# Build dev image with source code included
docker build -t pravega/gstreamer:latest-dev-with-source -f Dockerfile-dev-with-source .
# Build dev image with just binaries
docker build -t pravega/gstreamer:latest-dev -f Dockerfile-dev .
# Build base production image with necessary dependencies
docker build -t pravega/gstreamer:prod-base -f Dockerfile-prod-base .
# Build production image optimized binaries and no debug symbols (-O3 LTO)
docker build -t pravega/gstreamer:latest-prod -f Dockerfile-prod .
# Build production image optimized binaries and debug symbols
docker build -t pravega/gstreamer:latest-prod-dbg -f Dockerfile-prod-dbg .
