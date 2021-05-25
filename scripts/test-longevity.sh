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

NAMESPACE=examples
KEYCLOAK_SERVICE_ACCOUNT_FILE=${HOME}/keycloak-${NAMESPACE}.json
kubectl get secret ${NAMESPACE}-pravega -n ${NAMESPACE} -o jsonpath="{.data.keycloak\.json}" | base64 -d > ${KEYCLOAK_SERVICE_ACCOUNT_FILE}

ALLOW_CREATE_SCOPE=false
export pravega_client_tls_cert_path=/etc/ssl/certs/DST_Root_CA_X3.pem
PRAVEGA_CONTROLLER_URI=tls://pravega-controller.kubespray.nautilus-platform-dev.com:443
PRAVEGA_SCOPE=examples
PRAVEGA_STREAM=camera-claudio-01

pushd ${ROOT_DIR}/integration-test

cargo run --bin longevity-test -- \
--stream ${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
--controller ${PRAVEGA_CONTROLLER_URI} \
--keycloak-file "${KEYCLOAK_SERVICE_ACCOUNT_FILE}" \
|& tee -a /tmp/longevity-test.log
