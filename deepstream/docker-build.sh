#!/bin/bash
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
