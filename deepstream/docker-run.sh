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

ROOT_DIR=$(readlink -f $(dirname $0)/..)

# Allow X11 apps to access the screen.
xhost +

# Use --privileged to allow core dumps.
docker run -it --rm \
    --gpus all \
    --network host \
    --privileged \
    --log-driver json-file --log-opt max-size=10m --log-opt max-file=2 \
    -v ${ROOT_DIR}:/root/work/gstreamer-pravega \
    -v /tmp/.X11-unix:/tmp/.X11-unix \
    -v /dev/log:/dev/log \
    -e DISPLAY=${DISPLAY} \
    -e CUDA_VER=11.1 \
    -w /root/work/gstreamer-pravega \
    pravega/deepstream:latest
