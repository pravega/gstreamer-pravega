#!/usr/bin/env bash
gst-launch-1.0 \
-v \
uridecodebin uri=https://www.freedesktop.org/software/gstreamer-sdk/data/media/sintel_trailer-480p.webm \
! queue \
! videoscale \
! video/x-raw-rgb,width=320,height=200 \
! videoconvert \
! autovideosink \
|& tee go4.log
