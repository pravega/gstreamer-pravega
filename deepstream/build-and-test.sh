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
pushd ${ROOT_DIR}
cargo build --package gst-plugin-pravega --locked --release
cargo build --package pravega_protocol_adapter --locked --release
sudo ln -f -s -v ${ROOT_DIR}/target/release/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
sudo ln -f -s -v ${ROOT_DIR}/target/release/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/
gst-inspect-1.0 pravega

echo "[message-broker]" > /tmp/msgapi-config.txt &&
echo "keycloak-file = ${KEYCLOAK_SERVICE_ACCOUNT_FILE}" >> /tmp/msgapi-config.txt

export ALLOW_CREATE_SCOPE=false
export GST_DEBUG=INFO,pravegasrc:LOG,pravegasink:LOG,pravegatc:TRACE,fragmp4pay:LOG,qtdemux:LOG,h264parse:LOG,v4l2:LOG
export INPUT_STREAM=camera-claudio-05
export LOG_LEVEL=10
export MSGAPI_CONFIG_FILE=/tmp/msgapi-config.txt
export OUTPUT_METADATA_STREAM=metadata-claudio-08
export RECOVERY_TABLE=recovery-table-1
export RUST_LOG=nvds_pravega_proto=trace,info

# GST_DEBUG=WARN gst-launch-1.0 -v pravegasrc controller=$PRAVEGA_CONTROLLER_URI stream=$PRAVEGA_SCOPE/$INPUT_STREAM \
# allow-create-scope=false keycloak-file=$KEYCLOAK_SERVICE_ACCOUNT_FILE \
# start-mode=earliest \
# ! qtdemux ! \
# h264parse ! video/x-h264,alignment=au ! nvv4l2decoder ! identity silent=false ! fakesink

deepstream/python_apps/deepstream-pravega-demos/pravega-to-object-detection-to-pravega.py >& /tmp/app.log
