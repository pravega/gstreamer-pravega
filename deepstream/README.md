# Using NVIDIA DeepStream with Pravega

Pravega provides the following for NVIDIA DeepStream:

- Pravega Protocol Adapter for NVIDIA DeepStream

  This provides an implementation of NVIDIA DeepStream Message Broker for a Pravega event stream.
  It can be used to write metadata such as inferred bounding boxes to a Pravega event stream.
  It is not intended to write video or audio data.

- Docker container with NVIDIA DeepStream and Pravega

  Includes:
  - *GStreamer Plugins for Pravega* (pravegasrc, pravegasink) for writing video and audio data to Pravega
  - *Pravega Protocol Adapter for NVIDIA DeepStream*
  - *Pravega Video Server* which provides an HTTP Live Streaming web server view video and audio streams in a browser.

# Running Pravega DeepStream Examples in DeepStream Container

Build and start Docker container with DeepStream and Pravega.

```bash
user@host:~/gstreamer-pravega/deepstream$
./docker-build.sh
./docker-run.sh
root@host:~#
```

View GStreamer Plugins for Pravega.

```bash
gst-inspect-1.0 pravegasrc pravegasink
```

Run C++ DeepStream Test App 4 to detect objects and write JSON metadata to a Pravega event stream.

```bash
root@host:~#
cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test4 && \
PRAVEGA_PROTOCOL_ADAPTER_LOG=nvds_pravega_proto=trace,info \
deepstream-test4-app \
-i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so \
--conn-str "pravega://localhost:9090?fixed_routing_key=abc123" \
--topic examples/deepstream-test4 \
--cfg-file /dev/null
```

This will produce JSON metadata as shown below.
Note that much of this data is hard-coded and not actually inferred from the video (e.g. vehicle make, license plate number).
The bounding box (bbox) and object type (vehicle) are inferred from the video.
```json
{
  "messageid" : "ff7cc527-d019-4348-8b25-6df69c5b6dcc",
  "mdsversion" : "1.0",
  "@timestamp" : "2021-03-31T20:11:07.742Z",
  "place" : {
    "id" : "1",
    "name" : "XYZ",
    "type" : "garage",
    "location" : {
      "lat" : 30.32,
      "lon" : -40.549999999999997,
      "alt" : 100.0
    },
    "aisle" : {
      "id" : "walsh",
      "name" : "lane1",
      "level" : "P2",
      "coordinate" : {
        "x" : 1.0,
        "y" : 2.0,
        "z" : 3.0
      }
    }
  },
  "sensor" : {
    "id" : "CAMERA_ID",
    "type" : "Camera",
    "description" : "\"Entrance of Garage Right Lane\"",
    "location" : {
      "lat" : 45.293701446999997,
      "lon" : -75.830391449900006,
      "alt" : 48.155747933800001
    },
    "coordinate" : {
      "x" : 5.2000000000000002,
      "y" : 10.1,
      "z" : 11.199999999999999
    }
  },
  "analyticsModule" : {
    "id" : "XYZ",
    "description" : "\"Vehicle Detection and License Plate Recognition\"",
    "source" : "OpenALR",
    "version" : "1.0"
  },
  "object" : {
    "id" : "-1",
    "speed" : 0.0,
    "direction" : 0.0,
    "orientation" : 0.0,
    "vehicle" : {
      "type" : "sedan",
      "make" : "Bugatti",
      "model" : "M",
      "color" : "blue",
      "licenseState" : "CA",
      "license" : "XX1234",
      "confidence" : -0.10000000149011612
    },
    "bbox" : {
      "topleftx" : 597,
      "toplefty" : 481,
      "bottomrightx" : 711,
      "bottomrighty" : 569
    },
    "location" : {
      "lat" : 0.0,
      "lon" : 0.0,
      "alt" : 0.0
    },
    "coordinate" : {
      "x" : 0.0,
      "y" : 0.0,
      "z" : 0.0
    }
  },
  "event" : {
    "id" : "3509e9f8-0861-45d3-a8cb-43ed8ac0fc7a",
    "type" : "moving"
  },
  "videoPath" : ""
}
```

Run Python DeepStream Test App 4 to detect objects and write JSON metadata to a Pravega event stream..

```bash
root@host:~#
cd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test4 && \
python3 deepstream_test_4.py \
-i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so \
--conn-str "pravega://localhost:9090?fixed_routing_key=abc123" \
--topic examples/deepstream-test4 \
--cfg-file /dev/null
```

Use Pravega Flink Tools to view events in the Pravega stream.

```bash
user@host:~$
git clone https://github.com/pravega/flink-tools
cd flink-tools
git checkout blog1
./gradlew -PmainClass=io.pravega.flinktools.StreamToConsoleJob \
  flink-tools:run \
  --args="--input-stream examples/deepstream-test4 \
  --input-startAtTail true"
```

Capture video from an RTSP camera and write the video data to Pravega.
This uses a non-DeepStream container.

```bash
user@host:~/gstreamer-pravega$
docker/build-release.sh
export CAMERA_USER=admin
export CAMERA_IP=192.168.1.102
export CAMERA_PASSWORD=YourPassword
export STREAM=camera1
scripts/rtsp-camera-to-pravega-docker.sh
```

Start Pravega Video Server for viewing the video in a browser.

```bash
user@host:~/gstreamer-pravega$
scripts/pravega-video-server-docker.sh
```

Open your browser to: http://localhost:3030/player?scope=examples&stream=camera1

In a DeepStream container, read video from a Pravega stream, detect objects, and write inferred metadata to a Pravega stream

```bash
root@host:~#
cd ~/work/gstreamer-pravega/deepstream/python_apps/deepstream-pravega-demos
export STREAM=camera1
./pravega-to-object-detection-to-pravega.py --input-stream examples/${STREAM} --output-metadata-stream examples/metadata1
```

Capture video from an RTSP camera, detect objects, and show video with bounding boxes on the screen.

```bash
root@host:~#
cd ~/work/gstreamer-pravega/deepstream/python_apps/deepstream-pravega-demos
CAMERA_USER=admin
CAMERA_IP=192.168.1.102
CAMERA_PASSWORD=YourPassword
CAMERA_URI="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0"
./rtsp-camera-to-object-detection-to-screen.py --source-uri ${CAMERA_URI}
```

Capture video from an RTSP camera, detect objects, write original video data to a Pravega stream,
and write metadata to a Pravega event stream.

```bash
./rtsp-camera-to-object-detection-to-pravega.py --source-uri ${CAMERA_URI}
```

# Install on NVIDIA Jetson Nano

### Install dependencies

```bash
user@jetson:~$
sudo apt-get install \
    curl \
    gstreamer1.0-opencv \
    libatk1.0-dev \
    libcairo-dev \
    libges-1.0-dev \
    libgtk2.0-dev \
    libgtk-3-dev \
    libpango1.0-dev
```

### Install Rust

```bash
user@jetson:~/gstreamer-pravega/deepstream$
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

Add to *top* of ~/.bashrc:
```shell script
export PATH="$HOME/.cargo/bin:$PATH"
```

### Increase swap space

Add a swap file as shown here.

```bash
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo "/swapfile swap swap defaults 0 0" | sudo tee -a /etc/swapfile
```

For details, see https://linuxize.com/post/how-to-add-swap-space-on-ubuntu-18-04/.

### Enable VNC Server

See https://medium.com/@bharathsudharsan023/jetson-nano-remote-vnc-access-d1e71c82492b.

To set display resolution:
```
xrandr --fb 1600x960
```

### Building GStreamer Plugin for Pravega

```
user@jetson:~/gstreamer-pravega/gst-plugin-pravega$
cargo install cargo-deb
cargo deb
sudo dpkg -i target/debian/gst-plugin-pravega_*.deb
```

# Running Examples

```
user@jetson:~/gstreamer-pravega/deepstream$
STREAM=jetson1 scripts/camera-to-pravega.sh
```

```
user@jetson:~/gstreamer-pravega/deepstream/apps/deepstream-test1$
python3 deepstream_test_1.py ~/deepstream/deepstream/samples/streams/sample_720p.h264
```

# Running non-Pravega DeepStream Examples in DeepStream Container

## C Examples

```bash
xhost +
docker run --gpus all -it --rm --network host -v $PWD:/gstreamer-pravega -v /tmp/.X11-unix:/tmp/.X11-unix \
-e DISPLAY=$DISPLAY -e CUDA_VER=11.1 -w /opt/nvidia/deepstream/deepstream-5.1 nvcr.io/nvidia/deepstream:5.1-21.02-devel

root@df0a706a83b1:/opt/nvidia/deepstream/deepstream-5.1#

cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test1
make
./deepstream-test1-app /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264

cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test2
make
./deepstream-test2-app /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264

cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test3
make
./deepstream-test3-app file:///opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264
CAMERA_USER=admin
CAMERA_IP=192.168.1.102
URL1="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0"
URL2="file:///opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264"
./deepstream-test3-app ${URL1} ${URL2}

cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test4
make
GST_DEBUG=nvmsgbroker:TRACE ./deepstream-test4-app \
-i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto.so \
--conn-str "localhost;9092" --topic topic1 --cfg-file cfg_kafka.txt

cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test5
make
cp /gstreamer-pravega/deepstream/configs/test5_config_file_src_infer_1.txt configs/ && \
./deepstream-test5-app -c configs/test5_config_file_src_infer_1.txt -p 1

cp /gstreamer-pravega/deepstream/configs/test5_dec_infer-resnet_tracker_sgie_tiled_display_int8_1.txt configs/ && \
./deepstream-test5-app -c configs/test5_dec_infer-resnet_tracker_sgie_tiled_display_int8_1.txt -p 0

pushd /opt/nvidia/deepstream/deepstream/sources/libs/kafka_protocol_adaptor
make
cp /opt/nvidia/deepstream/deepstream/sources/libs/kafka_protocol_adaptor/libnvds_kafka_proto.so \
/opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto.so
ldconfig
cd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test4
make
GST_DEBUG=nvmsgbroker:TRACE ./deepstream-test4-app \
-i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto.so \
--conn-str "localhost;9092" --topic topic1 --cfg-file cfg_kafka.txt
```

## Python Examples

```bash
cd /opt/nvidia/deepstream/deepstream/lib && \
python3 setup.py install && \
apt update && \
apt install python3-gi python3-dev python3-gst-1.0 -y && \
cd /opt/nvidia/deepstream/deepstream/sources && \
git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps && \
cd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test1
python3 deepstream_test_1.py /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264

cd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test2
python3 deepstream_test_2.py /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264

cd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test3
CAMERA_USER=admin
CAMERA_IP=192.168.1.102
URL1="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0"
URL2="file:///opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264"
python3 deepstream_test_3.py ${URL1} ${URL2}

cd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test4
python3 deepstream_test_4.py -i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto.so \
--conn-str "localhost;9092" --topic topic1 --cfg-file cfg_kafka.txt

cp /opt/nvidia/deepstream/deepstream/sources/libs/kafka_protocol_adaptor/libnvds_kafka_proto.so \
/opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto_DEV.so
ldconfig
pushd /opt/nvidia/deepstream/deepstream/sources/deepstream_python_apps/apps/deepstream-test4
python3 deepstream_test_4.py -i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /opt/nvidia/deepstream/deepstream/lib/libnvds_kafka_proto_DEV.so \
--conn-str "localhost;9092" --topic topic1 --cfg-file cfg_kafka.txt
```

# Core Dumps in Docker

```bash
echo '/tmp/core.%h.%e.%t' > /proc/sys/kernel/core_pattern
ulimit -c unlimited
python3 ...
gdb /usr/bin/python3 /tmp/core*
bt
```

# References

- [Accelerated GStreamer](https://docs.nvidia.com/jetson/l4t/index.html#page/Tegra%2520Linux%2520Driver%2520Package%2520Development%2520Guide%2Faccelerated_gstreamer.html%23)
- [DeepStream Development Guide](https://docs.nvidia.com/metropolis/deepstream/dev-guide/index.html)
- https://dev.to/mizutani/how-to-get-core-file-of-segmentation-fault-process-in-docker-22ii
