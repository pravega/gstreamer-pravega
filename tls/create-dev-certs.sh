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

# Create TLS key/cert pairs for development only.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

pushd ${ROOT_DIR}/tls

# Create CA.
openssl genrsa -out ca.key 4096
openssl req -new -x509 -key ca.key -out ca.crt -days 7300 \
  -subj "/CN=DEVELOPMENT-ca"
openssl x509 -inform pem -in ca.crt -noout -text >> ca.crt

# Create TLS key pair - localhost.
SSLCN=localhost
openssl genrsa -out ${SSLCN}.key 4096
openssl req -new -key ${SSLCN}.key -out ${SSLCN}.csr \
  -subj "/CN=${SSLCN}"
openssl x509 -req -days 3650 -in ${SSLCN}.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out ${SSLCN}.crt
openssl x509 -inform pem -in ${SSLCN}.crt -noout -text >> ${SSLCN}.crt

popd
