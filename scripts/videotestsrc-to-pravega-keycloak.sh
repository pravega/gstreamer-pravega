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

# TODO: For an unknown reason, the timestamp appears to progress faster than real time.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:INFO,basesink:INFO
export RUST_BACKTRACE=1
export pravega_client_tls_cert_path=/etc/ssl/certs/ca-certificates.crt
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-tls://pravega-controller.kubespray.nautilus-platform-dev.com:443}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-false}
SIZE_SEC=10
FPS=30

NAMESPACE=${PRAVEGA_SCOPE}
KEYCLOAK_SERVICE_ACCOUNT_FILE=${HOME}/keycloak.json
kubectl get secret ${NAMESPACE}-pravega -n ${NAMESPACE} -o jsonpath="{.data.keycloak\.json}" | base64 -d > ${KEYCLOAK_SERVICE_ACCOUNT_FILE}

gst-launch-1.0 \
-v \
videotestsrc name=src is-live=false do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=1920,height=1280,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! x264enc key-int-max=${FPS} speed-preset=ultrafast bitrate=2000 \
! mpegtsmux alignment=-1 \
! pravegasink \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  keycloak-file=${KEYCLOAK_SERVICE_ACCOUNT_FILE} \
  seal=false \
  sync=false \
  allow-create-scope=${ALLOW_CREATE_SCOPE}
