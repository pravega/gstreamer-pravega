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
TO_IMAGE_NAME=devops-repo.isus.emc.com:8116/nautilus/deepstream-jupyter
TO_TAG=0.0.2
TO_IMAGE=${TO_IMAGE_NAME}:${TO_TAG}
RUST_JOBS=${RUST_JOBS:-4}

docker build \
    -t ${TO_IMAGE} \
    -t ${TO_IMAGE_NAME}:latest \
    --build-arg FROM_IMAGE=${FROM_IMAGE} \
    --build-arg RUST_JOBS=${RUST_JOBS} \
    -f ${ROOT_DIR}/jupyter/deepstream-jupyter.Dockerfile \
    ${ROOT_DIR}

docker push ${TO_IMAGE}
