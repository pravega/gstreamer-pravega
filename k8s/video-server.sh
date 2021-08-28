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
ROOT_DIR=$(readlink -f $(dirname $0)/..)
: ${1?"You must specify the values.yaml file."}
VALUES_FILE="$1"
shift
RELEASE_NAME=$(basename "${VALUES_FILE}" .yaml)
NAMESPACE=${NAMESPACE:-examples}

if [[ "${UNINSTALL}" == "1" ]]; then
    helm del -n ${NAMESPACE} ${RELEASE_NAME} || true
fi

if [[ "${INSTALL}" != "0" ]]; then
    helm upgrade --install --debug \
        ${RELEASE_NAME} \
        ${ROOT_DIR}/k8s/charts/video-server \
        --namespace ${NAMESPACE} \
        -f "${VALUES_FILE}" \
        $@
fi
