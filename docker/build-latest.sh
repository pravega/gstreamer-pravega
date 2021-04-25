#!/bin/bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

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
