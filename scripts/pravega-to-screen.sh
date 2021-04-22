#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:INFO,mpegtsbase:4,mpegtspacketizer:4,GST_TRACER:7"
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
pravegasrc \
<<<<<<< HEAD
<<<<<<< HEAD
=======
  stream=${PRAVEGA_SCOPE}/${STREAM} \
=======
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
>>>>>>> pravega-to-screen.sh now uses PRAVEGA_STREAM
  controller=${PRAVEGA_CONTROLLER_URI} \
>>>>>>> Change scripts to use PRAVEGA_CONTROLLER_URI and PRAVEGA_SCOPE
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
<<<<<<< HEAD
  controller=${PRAVEGA_CONTROLLER} \
  keycloak-file=\"${KEYCLOAK_FILE}\" \
  stream=${SCOPE}/${STREAM} \
=======
  start-pts-at-zero=true \
>>>>>>> Add start-pts-at-zero=true to pravega-to-screen*.sh
  $* \
! decodebin \
! videoconvert \
! textoverlay "text=from ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=true \
|& tee /tmp/pravega-to-screen.log
