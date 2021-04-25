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
FROM_IMAGE_NAME=nvcr.io/nvidia/deepstream
FROM_TAG=${FROM_TAG:-5.1-21.02-devel}
FROM_IMAGE=${FROM_IMAGE_NAME}:${FROM_TAG}
TO_IMAGE_NAME=pravega/deepstream
TO_TAG=${FROM_TAG}-pravega
TO_IMAGE=${TO_IMAGE_NAME}:${TO_TAG}

docker build \
    -t ${TO_IMAGE} \
    -t ${TO_IMAGE_NAME}:latest \
    --build-arg FROM_IMAGE=${FROM_IMAGE}\
    -f ${ROOT_DIR}/deepstream/pravega-dev.Dockerfile \
    ${ROOT_DIR}
