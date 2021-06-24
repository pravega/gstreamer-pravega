<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->
# GStreamer Plugins for Pravega

This repository contains plugins to read and write Pravega streams using [GStreamer](https://gstreamer.freedesktop.org/).

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

Here is a typical pipeline, which will obtain video from a camera, compress using H.264, encapsulate
in an MPEG Transport Stream, and write to a Pravega stream.
```
v4l2src (camera) -> x264enc -> mpegtsmux ->
pravegasink stream=examples/mystream controller=127.0.0.1:9090
```

## Pravega Source (pravegasrc)

The Pravega Source reads a series of byte buffers from a Pravega byte stream and delivers it to downstream components.
It is guaranteed to read the byte buffers in the same order in which they were written by the Pravega Sink.
Buffer timestamps (PTS) are also maintained.

The Pravega Source is seekable by absolute time.
The index is used to efficiently identify the offset to begin reading at.
Additionally, the Pravega Source will respond to seekable queries by providing the first and last timestamps in the time index.

Here is a typical pipeline, which will read an MPEG Transport Stream from a Pravega stream,
decode the video, and display it on the screen.
```
pravegasrc stream=examples/mystream controller=127.0.0.1:9090 ->
tsdemux -> h264parse -> avdec_h264 -> autovideosink (screen)
```

## Concurrent use of Pravega Sink and Pravega Source

It is common to have one process write to a Pravega Sink while one or more other processes across
a network read from the same Pravega stream using the Pravega Source.
Tail reads are able to achieve around 20 ms of end-to-end latency (less than 1 frame).
Using the Pravega Video Player, a user can seamlessly adjust the playback position from any point in the past to the current time.

## Generic GStreamer Buffers

Arbitrary GStreamer buffers can be stored and transported using Pravega by utilizing the gdppay and gdpdepay elements.

# Getting Started

## Getting Started with Ubuntu

GStreamer 1.18.0 and 1.18.4 have been tested and are recommended. Version 1.18.0 comes standard with Ubuntu version 20.10.

### Clone this Repository

```bash
git clone --recursive https://github.com/pravega/gstreamer-pravega
cd gstreamer-pravega
git submodule update --recursive --init
```

### Install GStreamer

```bash
sudo apt-get install \
    gstreamer1.0-tools \
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

For more details, refer to https://github.com/sdroege/gstreamer-rs.

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
rustup update
```

Add to ~/.bashrc:
```
export PATH="$HOME/.cargo/bin:$PATH"
```

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
You can use the `ifconfig` command to find the IP address of the eth0 or ensXX interface.

```bash
cd pravega-docker
export HOST_IP=x.x.x.x
export PRAVEGA_LTS_PATH=/tmp/pravega-lts
docker-compose down && \
sudo rm -rf ${PRAVEGA_LTS_PATH} && \
docker-compose up -d
cd ..
```

You must also create the Pravega scope. This can be performed using the REST API.
```
curl -X POST -H "Content-Type: application/json" -d '{"scopeName":"examples"}' http://localhost:10080/v1/scopes
```

You can view the Pravega logs with `docker-compose logs --follow`.

You can view the stream files stored on long-term storage (LTS) with `ls -h -R ${PRAVEGA_LTS_PATH}`.

## Docker Containers

Docker containers can be built with and without NVIDIA DeepStream.
The containers without DeepStream are based on a newer version of GStreamer.

- [Standard Docker Containers](docker/README.md)
- [Docker Containers with NVIDIA DeepStream](deepstream/README.md)

## Examples

When you run any of these examples for the first time, the Rust build system, Cargo, will download and build all dependencies.

### Synthetic video to Pravega

Generate synthetic video data, compress it using H.264, wrap it in an MPEG Transport Stream, and write to a Pravega stream.

```bash
PRAVEGA_STREAM=mystream1 scripts/videotestsrc-to-pravega.sh
```

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

### Additional Examples

You'll find a variety of other examples in [apps/src/bin](apps/src/bin) and
[scripts](scripts).

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

# (Optional) Build GStreamer from Source

Use this procedure to build GStreamer from source.
If you are using Ubuntu 20.10 or Docker, this is not required nor recommended.

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

# Testing

## Automated Tests

This will run unit and integration tests. It will start and stop a temporary Pravega standalone instance.

```bash
scripts/test-all.sh
...
test-all.sh: All tests completed successfully.
```

# Implementation Details

## Storing Media in Pravega

GStreamer Plugins for Pravega stores video and/or audio media in a Pravega byte stream.
A single write (referred to as an event) is always 8 MiB or less and it is atomic.
Each event includes a 20 byte header followed by a user-defined sequence of bytes (payload).
The header includes the length of the event, a 64-bit timestamp, and some flags.

The timestamp counts the number of nanoseconds since the epoch 1970-01-01 00:00 TAI (International Atomic Time).
This definition is used to avoid problems with the time going backwards during positive leap seconds.
As of 2020-01-09, TAI is exactly 37 seconds ahead of UTC.
This offset will change when additional leap seconds are scheduled.
This 64-bit counter will wrap in the year 2554.
This timestamp reflects the sampling instant of the first octet in the payload, as in RFC 3550.
For video frames, the timestamp will reflect when the image was captured by the camera.
This allows different streams to be correlated precisely.

Typically, the payload will be a single 188-byte frame of an MPEG transport stream
or a single fragment of a fragmented MP4 (fMP4).
An fMP4 fragment can be as small as a single video frame.
For most use cases, fMP4 is recommended.
fMP4 will store the PTS (presentaton timestamp) and DTS (decode timestamp) values with sufficient resolution
and range so that wrapping is not a practical concern.
When using fMP4, the timestamp in the event header is actually redundant with the PTS.

For details, see `EventWriter` in [event_serde.rs](pravega-video/src/event_serde.rs).

## The Media Index

GStreamer Plugins for Pravega writes a separate Pravega byte stream containing a time-based index,
allowing rapid seeks to any timestamp.
Each record in the index contains a 64-bit timestamp, a byte offset into the associated media stream, and some flags.
Typically, only video key frames are indexed.

For details, see `IndexRecordWriter` in [index.rs](pravega-video/src/index.rs).

# Time in GStreamer

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

## How Time is Used

1. Synchronizing with other streams, accurate to within a video/audio sample
   a. Other GStreamer streams
   b. Other types of sensors (e.g. accelerometer)
2. Seeking to UTC

# Update Dependencies

```bash
pushd gst-plugin-pravega ; cargo update ; popd
pushd pravega-video ; cargo update ; popd
pushd pravega-video-server ; cargo update ; popd
pushd apps ; cargo update ; popd
```

# GStreamer Pipeline Failure Recovery

## Transaction Coordinator

Initially, we will provide AT-LEAST-ONCE only, and a single input/output pad pair.
This may produce duplicate events but this can be bounded by the flush frequency.
Later, we will add the features needed for EXACTLY-ONCE and multiple input/output pad pairs.

### nvmsgbroker (Pravega Event Writer)

This is based on pravega-to-object-detection-to-pravega.py.

```
pravegasrc -> ...nvinfer... -> nvmsgconv -> transactioncoordinator -> nvmsgbroker
```

### pravegasink (Pravega Byte Stream Writer)

```
pravegasrc -> ...nvinfer... -> nvdsosd -> x264enc -> mpegtsmux -> transactioncoordinator -> pravegasink
```

Unlike the event writer, we can easily re-read the data written to the destination stream because it will be in a stream by itself.
However, using this ability would make failure recovery difficult.
Instead, we will assume that we can use a transaction to write to the Pravega byte stream.
This is likely possible since pravegasrc and pravegasink use an event encoding that is compatible with the Pravega event stream writer and reader.
With this assumption, failure recovery of pravegasink becomes the same as nvmsgbroker.

### Multiple Inputs and Outputs

It is also possible that we want to write both the metadata and video data to Pravega exactly-once.

```
pravegasrc -> ...nvinfer... -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker
                             \ nvdsosd -> x264enc -> mpegtsmux -/                  \ \--- pravegasink
                                                                                    \---- pravegasink
```

Multiple pravegasrc can be combined in a single pipeline for the sole purpose of batch processing in the GPU.
Each section of the pipeline is independent except at `nvstreammux -> nvinfer -> nvstreamdemux` where they must be combined.
These can use independent transaction coordinators and they can have independent PTS.

```
pravegasrc A -> ...nvstreammux -> nvinfer -> nvstreamdemux -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker A
                    /                                   \   \ nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink A
pravegasrc B -> .../                                     \--- nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker B
                                                          \-- nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink B
```

It is also possible that we want to perform inference on multiple video frames and produce an output.
This might be useful if the video feeds are cameras pointing at the same area from different angles (or 3D cameras), and we want to build a 3D model.

```
pravegasrc L -> ...nvstreammux -> nvinfer -> nvstreamdemux -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker L+R
                    /                                       \ nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink L+R
pravegasrc R -> .../
```

### Implementation

- In-memory State:
  - pts:
    - u64 which will equal the minimum PTS across inputs
  - active_transactions:
    - (future) list of active transactions
  - ready_to_flush:
    - ordered blocking queue of (`pts`, (future) `transactions`)
    - Events written to the transactions will have a timestamp strictly less than `pts`.

#### Chain function

Below describes the chain function in the Transaction Coordinator (TC).

- (future) Buffers from inputs will be queued (or inputs blocked) as needed to ensure that all buffers are processed in PTS order.
- Calculate `new_pts` = minimum PTS across all inputs.
- If `new_pts` is greater than `pts`.
  - Set `pts_changed` = true.
  - Set `pts` = `new_pts`.
- Determine when prior open transaction should be committed.
  This should be at a frame boundary, or equivalently `pts_changed` is true.
  We can also require the PTS to change by a minimum amount.
- If we should commit:
  - Add record to `ready_to_flush`:
    - `pts`: from new buffer
    - (future) `transactions`: from `active_transactions`
  - (future) Empty `active_transactions`.
  - (future) Begin new transactions and populate `active_transactions`.
  - (future) Notify each output to flush any internal buffers and use the new transactions.
    There is no need to flush the Pravega transactions at this point.
    - nvmsgbroker
      - Send custom event to use the new transaction.
    - pravegasink
      - Send custom event or buffer to use the new transactions (1 for data, 1 for index).
- Chain to outputs.
  - Write to Pravega asynchronously or synchronously.

### Commit thread

- Persistent State:
  - list of (`pts`, (future) `transaction_ids`)
  - This record indicates that we can recover a failed pipeline by commiting `transaction_ids` and then seeking to `pts`.
    A video decoder will need to clip its output to ensure that the first buffer has a PTS equal or greater than `pts`.

This thread will run in the background.

- Repeat forever:
  - Perform failure recovery if previous iteration did not succeed (but only seek the first time).
  - Read a record (`pts`, `transactions`) from the queue `ready_to_flush`.
  - Flush all transactions.
  - Atomically update the persistent state by appending the record (`pts`, (future) `transactions_ids`).
  - (future) Commit all transactions.
  - (future) Atomically update the persistent state by updating the record to have an empty list of `transaction_ids`.
    This avoids problems with committed transactions that expire before the pipeline runs again.

### Failure recovery

- Determine last recorded persistent state.
- (future) For each record (`pts`, `transactions_ids`):
  - Commit all transactions.
- Seek all inputs to `pts`.
  - pravegasrc will find the random access point at or immediately before `pts`.
  - Video decoders must clip output at exact `pts`.
  - Video encoders will start encoding at exact `pts`.
  - Can TC element perform the seek?

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
