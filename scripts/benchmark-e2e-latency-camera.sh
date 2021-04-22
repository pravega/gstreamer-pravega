#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
popd
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
export RUST_BACKTRACE=1

PRAVEGA_STREAM=${PRAVEGA_STREAM:-$(uuidgen)}
BITRATE_KILOBITS_PER_SEC=200
export GST_DEBUG="pravegasrc:4,timestampremove:5,pravegasink:5,mpegtsbase:4,mpegtspacketizer:4"

export GST_DEBUG_FILE=trace.log

gst-launch-1.0 \
-v \
  pravegasrc stream=examples/${PRAVEGA_STREAM} \
! timestampremove \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! textoverlay "text=from ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=false \
&

sleep 2s

export GST_DEBUG_FILE=

gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE num_buffers=1000 \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! tee name=t \
t. ! queue \
   ! x264enc tune=zerolatency bitrate=${BITRATE_KILOBITS_PER_SEC} \
   ! mpegtsmux \
   ! tee name=ts \
   ts. ! queue \
       ! timestampadd \
       ! pravegasink stream=examples/${PRAVEGA_STREAM} \
   ts. ! queue \
       ! tsdemux \
       ! h264parse \
       ! avdec_h264 \
       ! videoconvert \
       ! textoverlay "text=camera encode+decode" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
       ! autovideosink sync=false \
t. ! queue2 \
   ! textoverlay "text=camera to ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
   ! autovideosink sync=false \
&

wait
