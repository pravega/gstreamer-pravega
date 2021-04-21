#!/usr/bin/env bash

# TODO: For an unknown reason, the timestamp appears to progress faster than real time.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
#cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:TRACE,basesink:INFO
export RUST_BACKTRACE=1
export pravega_client_auth_method=Bearer
export pravega_client_auth_keycloak=/home/luis/keycloak.json
export pravega_client_tls_cert_path=/etc/ssl/certs/DST_Root_CA_X3.pem
STREAM=${STREAM:-test1}
SIZE_SEC=10
FPS=30

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
! pravegasink stream=rungpu/${STREAM} controller=pravega-controller.kubespray.nautilus-platform-dev.com:443 seal=false sync=false
