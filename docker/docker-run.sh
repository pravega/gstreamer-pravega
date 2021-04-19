#!/bin/bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)

# Use --privileged to allow core dumps.
docker run -it --rm \
    --network host \
    --privileged \
    --log-driver json-file --log-opt max-size=10m --log-opt max-file=2 \
    pravega/gstreamer:pravega-dev
