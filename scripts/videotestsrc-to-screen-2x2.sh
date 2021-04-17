#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
FPS=30
WIDTH=320
HEIGHT=240
CAPS="video/x-raw,width=$WIDTH,height=$HEIGHT"

gst-launch-1.0 \
compositor name=comp \
sink_0::xpos=0 sink_0::ypos=0 sink_0::width=$WIDTH sink_0::height=$HEIGHT \
sink_1::xpos=$WIDTH sink_1::ypos=0 sink_1::width=$WIDTH sink_1::height=$HEIGHT \
sink_2::xpos=0 sink_2::ypos=$HEIGHT sink_2::width=$WIDTH sink_2::height=$HEIGHT \
sink_3::xpos=$WIDTH sink_3::ypos=$HEIGHT sink_3::width=$WIDTH sink_3::height=$HEIGHT \
! autovideosink \
videotestsrc is-live=true pattern=smpte \
! $CAPS \
! comp. \
videotestsrc is-live=true pattern=ball \
! $CAPS \
! comp. \
videotestsrc is-live=true pattern=zone-plate kx2=20 ky2=20 kt=1 \
! $CAPS \
! comp. \
videotestsrc is-live=true pattern=snow \
! $CAPS \
! comp.
