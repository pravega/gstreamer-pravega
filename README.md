<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->
# GStreamer Plugins for Pravega

This repository contains plugins to read and write Pravega streams using [GStreamer](https://gstreamer.freedesktop.org/).

# Contents

- [GStreamer Plugins for Pravega](#gstreamer-plugins-for-pravega)
- [Contents](#contents)
- [What is GStreamer?](#what-is-gstreamer)
- [GStreamer Plugins for Pravega](#gstreamer-plugins-for-pravega-1)
  - [Pravega Sink (pravegasink)](#pravega-sink-pravegasink)
  - [Pravega Source (pravegasrc)](#pravega-source-pravegasrc)
  - [Pravega Transaction Coordinator (pravegatc)](#pravega-transaction-coordinator-pravegatc)
  - [Timestamp Convert (timestampcvt)](#timestamp-convert-timestampcvt)
  - [Fragmented MP4 Payloader (fragmp4pay)](#fragmented-mp4-payloader-fragmp4pay)
  - [Concurrent use of Pravega Sink and Pravega Source](#concurrent-use-of-pravega-sink-and-pravega-source)
  - [Generic GStreamer Buffers](#generic-gstreamer-buffers)
- [Getting Started](#getting-started)
  - [Getting Started with Ubuntu](#getting-started-with-ubuntu)
    - [Install GStreamer and Dependencies](#install-gstreamer-and-dependencies)
    - [Build GStreamer from Source (Not Recommended)](#build-gstreamer-from-source-not-recommended)
    - [Clone this Repository](#clone-this-repository)
    - [Install Rust](#install-rust)
    - [Install IDE (Optional)](#install-ide-optional)
    - [Build Pravega from Source (Optional)](#build-pravega-from-source-optional)
    - [Run Pravega](#run-pravega)
  - [Build and Install GStreamer Plugins for Pravega](#build-and-install-gstreamer-plugins-for-pravega)
  - [Examples](#examples)
    - [Synthetic Video to Pravega](#synthetic-video-to-pravega)
    - [Play Video from Pravega](#play-video-from-pravega)
    - [USB Camera to Pravega](#usb-camera-to-pravega)
    - [RTSP Camera to Pravega](#rtsp-camera-to-pravega)
    - [Pravega Video Player (Native)](#pravega-video-player-native)
    - [HTTP Live Streaming with Pravega Video Server](#http-live-streaming-with-pravega-video-server)
    - [RTSP Camera Simulator](#rtsp-camera-simulator)
    - [Export a Pravega Stream to a Fragmented MP4 File](#export-a-pravega-stream-to-a-fragmented-mp4-file)
    - [Export a Pravega Stream to a GStreamer Data Protocol (GDP) File](#export-a-pravega-stream-to-a-gstreamer-data-protocol-gdp-file)
    - [Import a GStreamer Data Protocol (GDP) File to a Pravega Stream](#import-a-gstreamer-data-protocol-gdp-file-to-a-pravega-stream)
    - [Additional Examples](#additional-examples)
  - [Docker Containers](#docker-containers)
  - [Truncating Streams](#truncating-streams)
- [Testing](#testing)
  - [Automated Tests](#automated-tests)
- [Architecture](#architecture)
  - [Video Compression and Encoding](#video-compression-and-encoding)
  - [MP4 Media Container Format](#mp4-media-container-format)
  - [Stream Truncation and Retention](#stream-truncation-and-retention)
  - [Seeking in a Video Stream](#seeking-in-a-video-stream)
  - [Change of Video Parameters](#change-of-video-parameters)
  - [Identification of Video Streams](#identification-of-video-streams)
  - [Timestamps](#timestamps)
  - [Storing and Retrieving Video in Pravega](#storing-and-retrieving-video-in-pravega)
    - [Data Stream Frame Format](#data-stream-frame-format)
    - [Data Stream Payload](#data-stream-payload)
    - [Index Stream Frame Format](#index-stream-frame-format)
  - [Time in GStreamer](#time-in-gstreamer)
    - [How Time is Used](#how-time-is-used)
  - [Pravega Video Server API](#pravega-video-server-api)
    - [Get HLS play list](#get-hls-play-list)
    - [Get media (video data)](#get-media-video-data)
  - [Failure Recovery](#failure-recovery)
- [How to Update Dependencies](#how-to-update-dependencies)
- [References](#references)
- [License](#license)

# What is GStreamer?

GStreamer is a pipeline-based multimedia framework that links together a wide variety of media processing systems to
complete complex workflows. For instance, GStreamer can be used to build a system that reads files in one format,
processes them, and exports them in another. The formats and processes can be changed in a plug and play fashion.

GStreamer supports a wide variety of media-handling components, including simple audio playback, audio and video
playback, recording, streaming and editing. The pipeline design serves as a base to create many types of multimedia
applications such as video editors, transcoders, streaming media broadcasters and media players.

It is designed to work on a variety of operating systems, e.g. Linux kernel-based operating systems, the BSDs,
OpenSolaris, Android, macOS, iOS, Windows, OS/400.

GStreamer is free and open-source software subject to the terms of the GNU Lesser General Public License (LGPL)

# GStreamer Plugins for Pravega

## Pravega Sink (pravegasink)

The Pravega Sink receives a series of byte buffers from an upstream element and writes the bytes to a Pravega byte stream.
Each buffer is framed with the buffer size and the absolute timestamp
(nanoseconds since 1970-01-01 00:00:00 International Atomic Time).
This can be used for storing a wide variety of multimedia content including H.264 video, AC3 audio, and
[MPEG transport streams](https://en.wikipedia.org/wiki/MPEG_transport_stream),
which can contain any number of audio and video channels.
Writes of buffers 8 MiB or less are atomic.

Since Pravega streams are append-only, seeking is not supported.

The Pravega Sink will also write an index stream associated with each data stream.
The index stream consists of 20-byte records containing the absolute timestamp and the byte offset.
A new index record is written for each key frame.

Pravega data and index streams can be truncated which means that all bytes earlier than a specified offset
can be deleted.

A Pravega Sink can be stopped (gracefully or ungracefully) and restarted, even when writing to the same stream.
Since Pravega provides atomic appends, it is guaranteed that significant corruption will not occur.

Arbitrary GStreamer buffers can be stored and transported using Pravega by utilizing the gdppay and gdpdepay elements.

## Pravega Source (pravegasrc)

The Pravega Source reads a series of byte buffers from a Pravega byte stream and delivers it to downstream components.
It is guaranteed to read the byte buffers in the same order in which they were written by the Pravega Sink.
Buffer timestamps (PTS) are also maintained.

The Pravega Source is seekable by absolute time.
The index is used to efficiently identify the offset to begin reading at.
Additionally, the Pravega Source will respond to seekable queries by providing the first and last timestamps in the time index.

## Pravega Transaction Coordinator (pravegatc)

This element can be used in a pipeline with a pravegasrc element to provide failure
recovery. A pipeline that includes these elements can be restarted after a failure
and the pipeline will resume from where it left off. The current implementation
is best-effort which means that some buffers may be processed more than once or
never at all. The pravegatc element periodically writes the PTS of the current
buffer to a Pravega table. When the pravegatc element starts, if it finds a PTS
in this Pravega table, it sets the start-timestamp property of the pravegasrc
element.

## Timestamp Convert (timestampcvt)

This element converts PTS timestamps for buffers.Use this for pipelines that will
eventually write to pravegasink.
This element drops any buffers without PTS.
Additionally, any PTS values that decrease will have their PTS corrected.

## Fragmented MP4 Payloader (fragmp4pay)

This element accepts fragmented MP4 input from mp4mux and emits buffers suitable
for writing to pravegasink.
Each output buffer will contain exactly one moof and one mdat atom in their
entirety. Additionally, output buffers containing key frames will be prefixed
with the ftype and moov atoms, allowing playback to start from any key frame.

## Concurrent use of Pravega Sink and Pravega Source

It is common to have one process write to a Pravega Sink while one or more other processes across
a network read from the same Pravega stream using the Pravega Source.
Tail reads are able to achieve around 20 ms of end-to-end latency (less than 1 frame).
Using the Pravega Video Player, a user can seamlessly adjust the playback position from any point in the past to the current time.

## Generic GStreamer Buffers

Arbitrary GStreamer buffers can be stored and transported using Pravega by utilizing the gdppay and gdpdepay elements.

# Getting Started

## Getting Started with Ubuntu

This section assumes that you are using Ubuntu Desktop version 21.04.
This comes with GStreamer 1.18.4 and is recommended.

### Install GStreamer and Dependencies

```bash
sudo apt-get install \
    curl \
    docker.io \
    docker-compose \
    git \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    libatk1.0-dev \
    libcairo-dev \
    libges-1.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    libgstrtspserver-1.0-dev \
    libgtk2.0-dev \
    libgtk-3-dev \
    libpango1.0-dev \
    libssl-dev
```

Run the following GStreamer command to confirm basic functionality.
You should see a window open that shows color bars and snow.

```bash
gst-launch-1.0 videotestsrc ! autovideosink
```

### Build GStreamer from Source (Not Recommended)

Use this procedure to build GStreamer from source.
This is **not** required **nor** recommended.

```bash
sudo apt install \
    bison \
    build-essential \
    flex \
    git \
    libpulse-dev \
    libsrtp2-dev \
    libvpx-dev \
    ninja-build \
    python-gi-dev \
    python3 \
    python3-gi \
    python3-gi-cairo \
    python3-pip
sudo pip3 install meson
git clone https://gitlab.freedesktop.org/gstreamer/gst-build
cd gst-build
./gst-worktree.py add gst-build-1.18 origin/1.18
cd gst-build-1.18
rm -rf builddir
meson builddir
ninja -C builddir
```

For more details, refer to https://www.collabora.com/news-and-blog/blog/2020/03/19/getting-started-with-gstreamer-gst-build/.

Use this command to open a shell with environment variables set to use this new build.
This allows you to use this build without installing it.

```bash
ninja -C builddir devenv
```

Optionally install this version.
This will be installed in `/usr/local` and it will be used instead of the version installed by your operating system.

```bash
sudo meson install -C builddir
sudo ldconfig
```

### Clone this Repository

```bash
git clone --recursive https://github.com/pravega/gstreamer-pravega
cd gstreamer-pravega
git submodule update --recursive --init
```

For more details, refer to https://github.com/sdroege/gstreamer-rs.

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
source $HOME/.cargo/env
rustup update
```

Add to ~/.bashrc:
```
export PATH="$HOME/.cargo/bin:$PATH"
```

### Install IDE (Optional)

For development, it is recommended to install [Visual Studio Code](https://code.visualstudio.com/docs/setup/setup-overview)
and the following extensions:
- Rust-in-peace Extension Pack

### Build Pravega from Source (Optional)

The following steps are optional. Run these steps if you wish to use a custom build of Pravega.

Install Java 11 and make it the default.
```bash
sudo update-alternatives --config java
```

```bash
pushd
git clone https://github.com/pravega/pravega
cd pravega
git checkout r0.9
./gradlew docker
docker tag pravega/pravega:latest pravega/pravega:0.9.0
docker tag pravega/bookkeeper:latest pravega/bookkeeper:0.9.0
popd
```

### Run Pravega

This will run a development instance of Pravega locally.
Note that the default *standalone* Pravega often used for development is likely insufficient for testing video because
it stores all data in memory and quickly runs out of memory.

In the command below, replace x.x.x.x with the IP address of a local network interface.
You can use the `ip address` command to find the IP address of the eth0 or ensXX interface.

```bash
cd pravega-docker
export HOST_IP=x.x.x.x
export PRAVEGA_LTS_PATH=/tmp/pravega-lts
sudo -E docker-compose down && \
sudo rm -rf ${PRAVEGA_LTS_PATH} && \
sudo -E docker-compose up -d
cd ..
```

You must also create the Pravega scope. This can be performed using the REST API.
```bash
curl -X POST -H "Content-Type: application/json" -d '{"scopeName":"examples"}' http://localhost:10080/v1/scopes
```

You can view the Pravega logs with `sudo -E docker-compose logs --follow` in the pravega-docker directory.

You can view the stream files stored on long-term storage (LTS) with `ls -h -R ${PRAVEGA_LTS_PATH}`.

## Build and Install GStreamer Plugins for Pravega

Use Cargo to build the GStreamer Plugins for Pravega.
If this is the first time, this may take 30 to 60 minutes.

```bash
cargo build --package gst-plugin-pravega --locked --release
```

Install the plugin.

```bash
sudo cp target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
```

Confirm that the plugin is available.

```bash
gst-inspect-1.0 pravega
```

You should see:

```
Plugin Details:
  Name                     pravega
  Description              GStreamer Plugin for Pravega
...
```

## Examples

### Synthetic Video to Pravega

Generate synthetic video data, compress it using H.264, wrap it in an MP4, and write to a Pravega stream.

```bash
export GST_DEBUG=pravegasink:DEBUG
NANOS_SINCE_EPOCH_TAI=$(( $(date +%s%N) + 37000000000 ))
gst-launch-1.0 -v \
  videotestsrc name=src timestamp-offset=${NANOS_SINCE_EPOCH_TAI} \
! "video/x-raw,format=YUY2,width=640,height=480,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! queue \
! x264enc tune=zerolatency key-int-max=30 \
! mp4mux streamable=true fragment-duration=1 \
! fragmp4pay \
! queue \
! pravegasink \
  stream=examples/my-stream \
  buffer-size=1024 \
  sync=true \
  ts-offset=-${NANOS_SINCE_EPOCH_TAI}
```

### Play Video from Pravega

This plays video from a Pravega stream using the basic `autovideosink` element.
Run this concurrently with the previous example to view "live" video.
You can control where to start with the `start-mode` and `start-utc` properties.
The property `sync=false` causes each frame to be displayed as soon as it is decoded, without regard to the timestamp.
This example does not provide any buffering, so playback may not be smooth.

```bash
export GST_DEBUG=pravegasrc:INFO
gst-launch-1.0 -v \
  pravegasrc \
  stream=examples/my-stream \
  start-mode=latest \
! decodebin \
! videoconvert \
! autovideosink sync=false
```

Now try different properties. Use `gst-inspect-1.0 pravegasink` to list the available properties.

### USB Camera to Pravega

Get video from a local USB camera, compress it using H.264, wrap it in an MPEG Transport Stream, and write to a Pravega stream.
This command can be run multiple times (but not concurrently) to append additional video frames to the Pravega stream.

```bash
PRAVEGA_STREAM=mystream1 scripts/camera-to-pravega.sh
```

### RTSP Camera to Pravega

Capture from RTSP camera and write video to a Pravega stream.

```bash
sudo apt install \
    python-gi-dev \
    python3 \
    python3-gi \
    python3-gi-cairo \
    python3-pip
pip3 install -r python_apps/requirements.txt
export CAMERA_PASSWORD=password
scripts/rtsp-camera-to-pravega-python.sh
```

### Pravega Video Player (Native)

Read video from a Pravega stream and play it on the screen.
This command can be executed before, during, or after `camera-to-pravega.sh`.
Multiple instances can be executed concurrently, even on different computers.

```bash
PRAVEGA_STREAM=mystream1 scripts/pravega-video-player.sh
```

### HTTP Live Streaming with Pravega Video Server

[HTTP Live Streaming (HLS)](https://en.wikipedia.org/wiki/HTTP_Live_Streaming)
allows all major browsers to view live video over an Internet connection.
HLS can achieve approximately 10 seconds of latency.
[Pravega Video Server](pravega-video-server) provides an HLS server that can
directly serve a Pravega stream containing an MPEG transport stream.

Generate synthetic video data that is suitable for HLS. This has key frames at 5 second intervals.

```bash
PRAVEGA_STREAM=mystream1 scripts/videotestsrc-to-pravega-hls.sh
```

Alternatively, generate synthetic video and audio data.

```bash
PRAVEGA_STREAM=mystream1 scripts/avtestsrc-to-pravega-hls.sh
```

Start Pravega Video Server.

```bash
scripts/generate-gap-file.sh
scripts/pravega-video-server.sh
```

Open your browser to:
http://localhost:3030/player?scope=examples&stream=mystream1

You may also specify a time window:
http://localhost:3030/player?scope=examples&stream=mystream1&begin=2021-01-25T00:00:00Z&end=2021-01-26T00:00:00Z

### RTSP Camera Simulator

The RTSP Camera Simulator can be used to simulate an RTSP camera using GStreamer.
RTSP players can connect to it and request live video, and it will send a video test pattern.

Build and run it using the following steps.

```bash
export CAMERA_PORT=8554
export CAMERA_USER=user
export CAMERA_PASSWORD=mypassword
scripts/rtsp-camera-simulator.sh
```

Alternatively, you may build and run it in a Docker container using the following steps:

```bash
export CAMERA_PORT=8554
export CAMERA_USER=user
export CAMERA_PASSWORD=mypassword
scripts/rtsp-camera-simulator-docker.sh
```

You can then use an RTSP player such as VLC to play the URL
`rtsp://user:mypassword@127.0.0.1:8554/cam/realmonitor?width=640&height=480`.

### Export a Pravega Stream to a Fragmented MP4 File

This will use the `gst-launch-1.0` application to run a GStreamer pipeline.
This pipeline will read video content from a Pravega stream, starting and stopping at specific timestamps,
and export the video in Fragmented MP4 format to a file in the shared project directory.

Run the following command in the Interactive Shell with GStreamer.

```bash
gst-launch-1.0 -v \
pravegasrc \
  stream=examples/my-stream \
  start-mode=timestamp \
  start-utc=2021-08-13T21:00:00.000Z \
  end-mode=timestamp \
  end-utc=2021-08-13T21:01:00.000Z \
! filesink \
  location=/tmp/export.mp4
```

If `end-mode` is `unbounded`, this will run continuously until the Pravega stream is sealed or deleted.
Otherwise, the application will terminate when the specified time range has been exported.

Run `gst-inspect-1.0 pravegasrc` to see the list of available properties for the `pravegasrc` element.

### Export a Pravega Stream to a GStreamer Data Protocol (GDP) File

The GStreamer Data Protocol (GDP) file format preserves buffer timestamps and other metadata.
When a Pravega stream is exported to a GDP file and later imported to a new Pravega stream,
the two Pravega streams will be identical.

```bash
gst-launch-1.0 -v \
pravegasrc \
  stream=examples/my-stream \
  start-mode=timestamp \
  start-utc=2021-08-13T21:00:00.000Z \
  end-mode=timestamp \
  end-utc=2021-08-13T21:01:00.000Z \
! "video/quicktime" \
! gdppay \
! filesink \
  location=/tmp/export.gdp
```

### Import a GStreamer Data Protocol (GDP) File to a Pravega Stream

```bash
gst-launch-1.0 -v \
filesrc \
  location=/tmp/export.gdp \
! gdpdepay \
! pravegasink \
  stream=examples/my-stream \
  sync=false
```

### Additional Examples

You'll find a variety of other examples in [apps/src/bin](apps/src/bin) and
[scripts](scripts).

## Docker Containers

Docker containers can be built with and without NVIDIA DeepStream.
The containers without DeepStream are based on a newer version of GStreamer.

- [Standard Docker Containers](docker/README.md)
- [Docker Containers with NVIDIA DeepStream](deepstream/README.md)

## Truncating Streams

Truncating a stream deletes all data in the stream prior to a specified byte offset.
Subsequent attempts to read the deleted data will result in an error.
Reads of non-truncated data will continue to succeed, using the same offsets used prior to the truncation.

Truncation can occur during writing and/or reading of the stream.
If the Pravega Video Player happens to be reading at a position that was truncated, it will
seek to the first available (non-truncated) position.

The Pravega Tools application provides a simple CLI to truncate both the data and index stream by time.
It can be used as shown below.

```
$ cd apps
$ cargo run --bin pravega-tools -- truncate-stream --scope examples --stream mystream1 --age-days 0.5
Truncating stream examples/mystream1 at 2020-10-08 06:12:40.747 UTC (1602137560747733)
Truncating prior to (IndexRecord { timestamp: 1602137007833949, offset: 192809376 }, 23280)
Index truncated at offset 23280
Data truncated at offset 192809376
```

# Testing

## Automated Tests

This will run unit and integration tests. It will start and stop a temporary Pravega standalone instance.

```bash
scripts/test-all.sh
...
test-all.sh: All tests completed successfully.
```

# Architecture

## Video Compression and Encoding

To understand the architecture of video with Pravega, it is useful to know how video is compressed. A video is simply a sequence of images, often 30 images per second. An image in a video is often referred to as a frame. An image compression format such as JPEG can compress a single image by removing redundant information. However, in a sequence of images, there is also redundant information between successive images. Nearly all video compression algorithms achieve high compression rates by identifying and removing the redundant information between successive images and within each image. Therefore a single frame of a compressed video is usually a delta frame, which means that it only contains the differences from the previous frame. A delta frame by itself cannot be used to reconstruct a complete frame. However, a typical video stream will occasionally (perhaps once per second) contain key frames which contains all the information to construct a complete frame. The task of a video decoder is to begin decoding at a key frame and then apply successive delta frames to reconstruct each frame. A video decoder must necessarily maintain a state consisting of one or sometimes more frame buffers.

For details, see [A Guide to MPEG Fundamentals and Protocol Analysis](http://www.img.lx.it.pt/~fp/cav/Additional_material/MPEG2_overview.pdf).

## MP4 Media Container Format

Pravega will write video in the common fragmented MP4 container format. This is an efficient and flexible format and allows storage of H.264 video and most audio formats.

The built-in MP4 mux in GStreamer (mp4mux) can output fragmented MP4 but the output is not well-suited for Pravega because it only writes important headers at the start of a pipeline, making truncation and seeking challenging. Pravega provides a new GStreamer element called fragmp4pay that will duplicate the required headers at every indexed position. This makes truncation and seeking trivial, and the resulting MP4 will remain playable with standard MP4 players.

## Stream Truncation and Retention

Truncating a stream deletes all data in the stream prior to a specified byte offset. Subsequent attempts to read the deleted data will result in an error. Reads of non-truncated data will continue to succeed, using the same offsets used prior to the truncation. Truncation can occur during writing and/or reading of the stream.

Video streams written by GStreamer consist of a data stream and an index stream. These must be truncated carefully to ensure consistency.
Truncation will be periodically performed by the Pravega Sink GStreamer element as it writes video to the Pravega stream.
Video streams can have a retention policy by age, size, or both.
The generic Pravega retention policy mechanism will not be used for video streams written by GStreamer.
To conform with the HLS spec, the start of each fragment, and therefore each index position, must contain all video headers. This constraint is satisfied by careful indexing so it does not impact truncation.

## Seeking in a Video Stream

A common requirement for all video solutions is to allow seeking to a particular position in a video streams. For instance, a video player will often provide a seek control allowing the user to navigate to any time in the video.
Seeking will be by time in nanoseconds. In the case of Pravega, it is appropriate to seek by UTC or TAI, specified as the number of nanoseconds since the UTC epoch 1970-01-01 00:00:00 (either excluding or including leap seconds).
When seeking, the Pravega Source GStreamer element will locate a nearby timestamp in the index, obtain the offset, and then position the data reader at that offset. Because the element fragmp4pay was used to write the stream, it is guaranteed that playback can begin at this position.

## Change of Video Parameters

A Pravega video stream will typically be long lasting. A stream duration of several years would be reasonable. During this lifetime, it is possible that the video parameters (resolution, frame rate, codec, bit rate, etc.) will need to be changed.
This is accomodated by requiring all random-access points to start with the necessary headers. New encoding sessions will start with the discontinuity bit set to true.

## Identification of Video Streams

A Pravega stream can have any number of metadata tags associated with it. When the Pravega Sink GStreamer element writes a video stream,
it will contain the metadata tag `video`.

## Timestamps

GStreamer represents a time duration as the number of nanoseconds (1e-9 seconds), encoded with an unsigned 64-bit integer. This provides a range of 584 years which is well within the expected lifetime of any GStreamer application. Unfortunately, there are multiple standards for the epoch (time zero) and whether leap seconds are counted. The GStreamer Plugin for Pravega stores timestamps as the number of nanoseconds since 1970-01-01 00:00 TAI (International Atomic Time), including leap seconds. This convention allows video samples, audio samples, and other events to be unambiguously represented, even during a leap second.

By far, the most common representation of time in computers is the POSIX clock, which counts the number of seconds since 1970-01-01 00:00:00 UTC, except leap seconds. Thus, most computer clocks will go backward for 1 second when leap seconds occur. It is quite possible that a backward moving timestamp will cause problems with a system that demands frame-level precision. Leap seconds are scheduled 6 months in advance by an international organization when the Earth's rotation slows down (or speeds up) relative to the historical average.

Although leap seconds will continue to be a challenge for all other components (e.g. cameras, temperature sensors, Linux hosts), by using TAI in Pravega, we can at least unambiguously convert time stored in Pravega to UTC.

It is convenient to think of leap seconds much like Daylight Saving Time. Most computer systems avoid the 1 hour jumps in time during DST by storing time as UTC and converting to the user's local time only when displaying the time. When this same concept is used to handle leap seconds, we get TAI.

As of 1 January 2017, when another leap second was added, TAI is exactly 37 seconds ahead of UTC.

As a consequence of using TAI in GStreamer Plugin for Pravega, it will need to know the leap second schedule. As of the current version, it can assume a fixed 37 second offset but if a new leap second is scheduled, then it will need to be updated with the leap second schedule. As of 2021-08-25, a leap second has not been scheduled and it is possible that leap seconds will not be scheduled for years or even decades.

## Storing and Retrieving Video in Pravega

### Data Stream Frame Format

The Pravega Sink plugin for GStreamer writes video to a Pravega byte stream using the encoding below, which is defined in
[event_serde.rs](pravega-video/src/event_serde.rs).
The entire frame is appended to the Pravega byte stream atomically.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|          type_code (32-bit BE signed int, set to 0)           |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            event_length (32-bit BE unsigned int)              |
|    number of bytes from reserved to the end of the payload    |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                         |D|R|I|
|                    reserved (set to 0)                  |I|A|N|
|                                                         |S|N|D|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                                                               |
|                timestamp (64-bit BE unsigned int)             |
+               nanoseconds since 1970-01-01 00:00 TAI          +
|                    including leap seconds                     |
|                                                               |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                    payload (variable length)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

One tick mark represents one bit position.

- type code:
   The type code must be 0 which corresponds to pravega_wire_protocol::wire_commands::EventCommand.TYPE_CODE.
   This makes this byte stream compatible with a Pravega event stream reader.
- event length:
   This is the number of bytes from reserved to the end of the payload.
   Encoded as a 32-bit big-endian unsigned int.
- reserved:
   All reserved bits must be 0.
   These may be utilized in the future for other purposes.
- DIS - discontinuity indicator:
   True (1) if this event is or may be discontinuous from the previous event.
   This should usually be true for the first event written by a new process.
   It has the same meaning as in an MPEG transport stream.
- RAN - random access indicator:
   True (1) when the stream may be decoded without errors from this point.
   This is also known as IDR (Instantaneous Decoder Refresh).
   Usually, MPEG I-frames will have a true value for this field and all
   other events will have a false value.
- IND - include in index:
   If true (1), this event should be included in the index.
   Typically, this will equal random_access but it is possible
   that one may want to index more often for Low-Latency HLS or
   less often to reduce the size of the index.
- timestamp:
   The timestamp counts the number of nanoseconds since the epoch 1970-01-01 00:00 TAI (International Atomic Time).
   This definition is used to avoid problems with the time going backwards during positive leap seconds.
   If the timestamp is unknown or if there is ambiguity when converting from a UTC time source
   in the vicinity of a positive leap second, timestamp can be recorded as 0.
   As of 2021-08-25, TAI is exactly 37 seconds ahead of UTC.
   This offset will change when additional leap seconds are scheduled.
   This 64-bit counter will wrap in the year 2554.
   This timestamp reflects the sampling instant of the first octet in the payload, as in RFC 3550.
   For video frames, the timestamp will reflect when the image was captured by the camera.
   If DTS can differ from PTS, this timestamp should be the PTS.
   This allows different streams to be correlated precisely.
- payload:
   Can be 0 or more fragmented MP4 atoms, or any other payload.
   Writes of the entire frame (type code through payload) must be atomic,
   which means it must be 8 MiB or smaller.

For details, see `EventWriter` in [event_serde.rs](pravega-video/src/event_serde.rs).

### Data Stream Payload

It is recommended to store MP4 fragments in the payload of the data stream events. This provides the following features:

- Multiplexing of any number of video and audio channels in the same byte stream
- An additional time source mechanism to deal with clock drift
- Allows truncation and concatenation at any point

Typically, the payload will be a single fragment of a fragmented MP4 (fMP4).
An fMP4 fragment can be as small as a single video frame.
For most use cases, fMP4 is recommended.
fMP4 will store the PTS (presentaton timestamp) and DTS (decode timestamp) values with sufficient resolution
and range so that wrapping is not a practical concern.
When using fMP4, the timestamp in the event header is actually redundant with the PTS.

### Index Stream Frame Format

When the Pravega Sink GStreamer element writes a video stream, it will also periodically (usually once per second) write records to an index stream. The index stream is a Pravega byte stream. The index stream has the same name as the video stream, but with "-index" appended to it. The index provides a mapping from the timestamp to the byte offset. It it used for seeking, truncation, failure recovery, and efficiently generating an HTTP Live Streaming (HLS) playlist.

The index must be reliable in the sense that if it has a {timestamp, offset} pair, then it must be able to read from this offset in the video stream. When possible, Pravega will attempt to gracefully handle violations of these constraints.

The index and related data stream must satisfy the following constraints.

1. If the first record in the index has timestamp T1 and offset O1 (T1, O1),
   and the last record in the index has timestamp TN and offset ON (TN, ON),
   then the data stream can be read from offset O1 inclusive to ON exclusive.
   The bytes prior to O1 may have been truncated.
   All bytes between O1 and ON have been written to the Pravega server and,
   if written in a transaction, the transaction has been committed.
   However, it is possible that reads in this range may block for a short time
   due to processing in the Pravega server.
   Reads in this range will not block due to any delays in the writer.
2. All events in the data stream between O1 and ON will have a timestamp
   equal to or greater than T1 and strictly less than TN.
3. If there are no discontinuities, the samples in the stream were sampled
   beginning at time T1 and for a duration of TN - T1.
4. If index records 2 through N have DIS of 0, then it is guaranteed that
   the bytes between O1 and ON were written continuously.

The index uses the encoding below, which is defined in [index.rs](pravega-video/src/index.rs).

The entire frame is appended to the Pravega byte stream atomically.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                         |D|R|R|
|                    reserved (set to 0)                  |I|A|E|
|                                                         |S|N|S|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                                                               |
|                timestamp (64-bit BE unsigned int)             |
+               nanoseconds since 1970-01-01 00:00 TAI          +
|                    including leap seconds                     |
|                                                               |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                                                               |
|                 offset (64-bit BE unsigned int)               |
+                 byte offset into Pravega stream               +
|                                                               |
|                                                               |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

One tick mark represents one bit position.

- reserved, RES:
   All reserved bits must be 0.
   These may be utilized in the future for other purposes.
- DIS - discontinuity indicator
- RAN - random access indicator

For details, see `IndexRecordWriter` in [index.rs](pravega-video/src/index.rs).

## Time in GStreamer

```
------------------+------------------------------------------------------------------------------
Events            | Durations
------------------+------------------------------------------------------------------------------
                  |
1970-01-01 00:00  |   |--------- +---- +---------- +---------- +---------- +---------- +---------
(Stream Start)    |   |          |     |           |           |           |           |
                  |   |          |     |GstElement |GstSegment |           |GstSegment |persisted
                  |   |          |     |base time  |start      |           |time       |timestamp
                  |   |          |     |           |           |           |           |
Play              |   |          |     +---------- +-----------|GstSegment +---------- |
                  |   |realtime  |     |                       |position               |
                  |   |clock     |     |GstBuffer              |                       |
Segment Start     |   |          |     |DTS/PTS                |                       |
                  |   |          |     |                       |                       |
Buffer Start      |   |          |     +----------             +----------             +---------
                  |   |          |     |duration
Buffer End        |   |          |     +----------
                  |   |          |
Now               |   +--------- +----
                  |
------------------+------------------------------------------------------------------------------
```

When writing with timestamp-mode=realtime-clock (sink):
```
persisted timestamp = GstElement base time + GstBuffer PTS + UTC-TAI offset
```

When writing with timestamp-mode=ntp (sink):
```
persisted timestamp = GstBuffer PTS + 70 years 17 leap days + UTC-TAI offset
```

When reading (source):
```
GstSegment time = persisted timestamp of first buffer read
GstBuffer PTS = persisted timestamp - GstSegment time
```

### How Time is Used

1. Synchronizing with other streams, accurate to within a video/audio sample
   a. Other GStreamer streams
   b. Other types of sensors (e.g. accelerometer)
2. Seeking to UTC

## Pravega Video Server API

The Pravega Video Server is a component that allows all major web browsers to play historical and live video.
It is an HTTP web service that supports [HTTP Live Streaming](https://en.wikipedia.org/wiki/HTTP_Live_Streaming).

The browser retrieves video data from Pravega using the API described in this section.

### Get HLS play list

**Request:** GET /scopes/my_scope/streams/my_stream/m3u8?begin=2021-04-19T00:00:00Z&end=2021-04-20T00:00:00Z

To avoid very large responses, requests should include begin and end timestamps with a timespan of 24 hours or less.
Requests without a begin timestamp will start at the first index record.
Requests without an end timestamp will end at the last index record.

**Response:** m3u8 text file

The playlist will be generated on-demand based on data in the video index.

### Get media (video data)

**Request:** GET /scopes/my_scope/streams/my_stream/media?begin=0&end=12345

Requests must include a byte range. Allowed byte ranges are provided in the HLS play list.

**Response:** 1 or more MP4 fragments

## Failure Recovery

See [Failure Recovery](documentation/src/docs/failure-recovery.md).

# How to Update Dependencies

```bash
pushd gst-plugin-pravega ; cargo update ; popd
pushd pravega-video ; cargo update ; popd
pushd pravega-video-server ; cargo update ; popd
pushd apps ; cargo update ; popd
```

# References

- https://gstreamer.freedesktop.org/documentation/tutorials/index.html?gi-language=c
- https://github.com/sdroege/gstreamer-rs
- https://gstreamer.freedesktop.org/
- https://en.wikipedia.org/wiki/GStreamer
- https://mindlinux.wordpress.com/2013/10/23/time-and-synchronization-for-dummies-yes-you-edward-hervey/
- https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
- [MPEG Timing Model](http://www.bretl.com/mpeghtml/timemdl.HTM)
- [A Guide to MPEG Fundamentals and Protocol Analysis](http://www.img.lx.it.pt/~fp/cav/Additional_material/MPEG2_overview.pdf)
- [TSDuck, The MPEG Transport Stream Toolkit](https://tsduck.io/)
- [RFC 8216: HTTP Live Streaming](https://tools.ietf.org/html/rfc8216)
- [HTTP Live Streaming Overview](https://developer.apple.com/library/archive/documentation/NetworkingInternet/Conceptual/StreamingMediaGuide/Introduction/Introduction.html#//apple_ref/doc/uid/TP40008332-CH1-SW1)
- [ONVIF Streaming Spec](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec-v221.pdf)
- [RFC 3550: RTP: A Transport Protocol for Real-Time Applications](https://www.rfc-editor.org/rfc/rfc3550)
- [RFC 6184: RTP Payload Format for H.264 Video](https://datatracker.ietf.org/doc/html/rfc6184)
- [RFC 7826: Real-Time Streaming Protocol (RTSP)](https://www.rfc-editor.org/rfc/rfc7826.html)

# License

GStreamer Plugins for Pravega is 100% open source and community-driven. All components are available under [Apache 2 License](https://www.apache.org/licenses/LICENSE-2.0.html) on GitHub.
