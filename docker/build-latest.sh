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
docker build -t pravega/gstreamer:dev-downloaded -f dev.Dockerfile --target download .
docker build -t pravega/gstreamer:latest-dev -f dev.Dockerfile .

# Build production image optimized binaries and no debug symbols (-O3 LTO)
docker build -t pravega/gstreamer:latest-prod -f prod.Dockerfile --target prod .
# Build production image optimized binaries and debug symbols
docker build -t pravega/gstreamer:latest-prod-dbg -f prod.Dockerfile --target debug-prod .
