#!/usr/bin/env bash

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

ROOT_DIR=$(readlink -f -- "$(dirname -- "$0")/..")

export DATA_DIR=${DATA_DIR:-${ROOT_DIR}}

docker run \
-d \
-p 8888:8888 \
-e JUPYTER_ENABLE_LAB=yes \
-v "${ROOT_DIR}":/home/jovyan/gstreamer-pravega \
-v "${DATA_DIR}":/home/jovyan/data \
--name jupyter-notebook-gstreamer-pravega \
jupyter/scipy-notebook:6d42503c684f \
jupyter-lab \
--ip=0.0.0.0 \
--no-browser

sleep 5s

docker logs jupyter-notebook-gstreamer-pravega
