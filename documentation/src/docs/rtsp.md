---
title: RTSP
---

<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->

The rtspsrc element in GStreamer 1.16 does not set PTS to the NTP timestamp. GStreamer 1.18 does this as expected.

```shell
export CAMERA_PASSWORD=...
scripts/rtsp-camera-to-pravega-debug.sh
```

/tmp/rtsp-camera-to-pravega.log
```
0:00:01.693363663 10281 0x55975a408b20 INFO             pravegasink src/pravegasink/imp.rs:463:gstpravega::pravegasink::imp:<pravegasink0> timestamp_mode=Ntp
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: latency = 2000
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: ntp-sync = true
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: rfc7273-sync = false
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: ntp-time-source = running-time
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: drop-on-latency = true
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: max-rtcp-rtp-time-diff = 1000
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: max-ts-offset-adjustment = 0
/GstPipeline:pipeline0/GstRTSPSrc:rtspsrc0/GstRtpBin:manager: buffer-mode = none
0:00:07.933004018 10281 0x55975a408b20 INFO             pravegasink src/pravegasink/imp.rs:434:gstpravega::pravegasink::imp:<pravegasink0> Using clock_type=Realtime, time=449830:13:01.782184020, (1619388781782184020 ns)
Could not receive any UDP packets for 5.0000 seconds, maybe your firewall is blocking it. Retrying using a tcp connection.
0:00:16.044741555 10281 0x55975a7e1360 DEBUG                rtspsrc gstrtspsrc.c:7352:gst_rtspsrc_setup_streams_start:<rtspsrc0> transport is now RTP/AVP/TCP;unicast;interleaved=0-1
...
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = event   ******* (identity-from-rtspsrc:sink) E (type: stream-start (10254), GstEventStreamStart, stream-id=(string)df4220131b982e047b335f8c6927f4391d46e31aa74c85a2f10e8bac9601fc9f/0/video:0:0:RTP:AVP:96, flags=(GstStreamFlags)GST_STREAM_FLAG_NONE;) 0x7f0dd0003410
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = event   ******* (identity-from-rtspsrc:sink) E (type: caps (12814), GstEventCaps, caps=(GstCaps)"application/x-rtp\,\ media\=\(string\)video\,\ payload\=\(int\)96\,\ clock-rate\=\(int\)90000\,\ encoding-name\=\(string\)H264\,\ packetization-mode\=\(string\)1\,\ profile-level-id\=\(string\)640032\,\ sprop-parameter-sets\=\(string\)\"Z2QAMqw0yAJACj/wFuAgICgAAB9AAATiB0MABoLgABoLhd5caGAA0FwAA0Fwu8uFAA\\\=\\\=\\\,aO48MAA\\\=\"\,\ a-packetization-supported\=\(string\)DH\,\ a-rtppayload-supported\=\(string\)DH\,\ a-framerate\=\(string\)20.000000\,\ a-recvonly\=\(string\)\"\"\,\ ssrc\=\(uint\)526441769\,\ clock-base\=\(uint\)56617\,\ seqnum-base\=\(uint\)56617\,\ npt-start\=\(guint64\)0\,\ play-speed\=\(double\)1\,\ play-scale\=\(double\)1\,\ onvif-mode\=\(boolean\)false";) 0x7f0de8003e00
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = event   ******* (identity-from-rtspsrc:sink) E (type: segment (17934), GstEventSegment, segment=(GstSegment)"segment, flags=(GstSegmentFlags)GST_SEGMENT_FLAG_NONE, rate=(double)1, applied-rate=(double)1, format=(GstFormat)time, base=(guint64)0, offset=(guint64)0, start=(guint64)0, stop=(guint64)18446744073709551615, time=(guint64)0, position=(guint64)0, duration=(guint64)18446744073709551615;";) 0x7f0dfc067830
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (15 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004040 discont tag-memory , meta: none) 0x7f0dc4008c60
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (61 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dc4008d80
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (17 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dfc05ab40
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (30 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dc4008b40
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (27 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dc4008a20
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (18 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dfc05a5a0
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (1452 bytes, dts: none, pts: 0:00:00.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0dc4008ea0
...
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (936 bytes, dts: none, pts: 0:00:04.000000000, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0df81536c0
```

Above, PTS started at 0 and increased to around 4 seconds.
After around 10 seconds, rtspsrc determines the NTP time and PTS is set to the time since the NTP epoch 1900-01-01 00:00:00.
1063438 hours is approximately 121 years.

```
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (15 bytes, dts: none, pts: 1063438:35:17.991999999, duration: none, offset: -1, offset_end: -1, flags: 00004080 resync tag-memory , meta: none) 0x7f0df81535a0
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (27 bytes, dts: none, pts: 1063438:35:17.991999999, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0df81bec60
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (1452 bytes, dts: none, pts: 1063438:35:17.991999999, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0df81beea0
/GstPipeline:pipeline0/GstIdentity:identity-from-rtspsrc: last-message = chain   ******* (identity-from-rtspsrc:sink) (1452 bytes, dts: none, pts: 1063438:35:17.991999999, duration: none, offset: -1, offset_end: -1, flags: 00004000 tag-memory , meta: none) 0x7f0df81c0000

/GstPipeline:pipeline0/GstIdentity:from-mpegtsmux: last-message = chain   ******* (from-mpegtsmux:sink) (188 bytes, dts: none, pts: 1063438:35:18.391999999, duration: none, offset: -1, offset_end: -1, flags: 00002400 header delta-unit , meta: none) 0x7f0df81017e0
```

On next lines, PTS (4 seconds) is not a valid NTP timestamp so pravegasink records timestamp=None.

```
/GstPipeline:pipeline0/GstIdentity:from-queue: last-message = chain   ******* (from-queue:sink) (188 bytes, dts: none, pts: 0:00:04.000000000, duration: none, offset: -1, offset_end: -1, flags: 00002400 header delta-unit , meta: none) 0x7f0df809fc60
0:00:10.407215791 39737 0x5615ef3d14c0 LOG              pravegasink src/pravegasink/imp.rs:613:gstpravega::pravegasink::imp:<pravegasink0> render: timestamp=None, pts=0:00:04.000000000, base_time=449830:35:08.883342152, duration=--:--:--.---------, size=188
```

On next lines, pravegasink identifies a valid NTP timestamp.

```
/GstPipeline:pipeline0/GstIdentity:from-queue: last-message = chain   ******* (from-queue:sink) (188 bytes, dts: none, pts: 1063438:35:17.991999999, duration: none, offset: -1, offset_end: -1, flags: 00006400 header delta-unit tag-memory , meta: none) 0x7f0df80ad900
0:00:10.407534368 39737 0x5615ef3d14c0 LOG              pravegasink src/pravegasink/imp.rs:613:gstpravega::pravegasink::imp:<pravegasink0> render: timestamp=2021-04-25T22:35:17.991999999Z (1619390154991999999 ns), pts=1063438:35:17.991999999, base_time=449830:35:08.883342152, duration=--:--:--.---------, size=188
```
