#!/usr/bin/env bash
gst-launch-1.0 \
-v
souphttpsrc location=https://www.freedesktop.org/software/gstreamer-sdk/data/media/sintel_trailer-480p.webm \
! matroskademux name=d d.video_0 \
! matroskamux \
! filesink location=sintel_video.mkv \
|& tee go4.log
