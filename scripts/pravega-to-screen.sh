#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:INFO,mpegtsbase:4,mpegtspacketizer:4,GST_TRACER:7"
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
SCOPE=${SCOPE:-examples}
STREAM=${STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
pravegasrc \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  controller=${PRAVEGA_CONTROLLER} \
  keycloak-file=\"${KEYCLOAK_FILE}\" \
  stream=${SCOPE}/${STREAM} \
  $* \
! decodebin \
! videoconvert \
! textoverlay "text=from ${STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=false \
|& tee /tmp/pravega-to-screen.log
