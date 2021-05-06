#!/usr/bin/env python3

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

#
# Capture from RTSP camera and write video to a Pravega stream.
#

import configargparse as argparse
import ctypes
import distutils.util
import logging
import os
import signal
import sys
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


def bus_call(bus, message, loop):
    """Callback for GStreamer bus messages"""
    t = message.type
    if t == Gst.MessageType.EOS:
        logging.info("End-of-stream")
        loop.quit()
    elif t == Gst.MessageType.WARNING:
        err, debug = message.parse_warning()
        logging.warning("%s: %s" % (err, debug))
    elif t == Gst.MessageType.ERROR:
        err, debug = message.parse_error()
        logging.error("%s: %s" % (err, debug))
        loop.quit()
    elif t == Gst.MessageType.ELEMENT:
        details = message.get_structure().to_string()
        logging.info("%s: %s" % (message.src.name, str(details),))
    return True


def str2bool(v):
    return bool(distutils.util.strtobool(v))


def main():
    parser = argparse.ArgumentParser(
        description="Capture from RTSP camera and write video to a Pravega stream",
        auto_env_var_prefix="")
    # Note that below arguments can be passed through the environment such as PRAVEGA_CONTROLLER_URI.
    parser.add_argument("--allow-create-scope", type=str2bool, default=True)
    parser.add_argument("--buffer-size-mb", type=float, default=10.0, help='Buffer size in MiB')
    parser.add_argument("--camera-address")
    parser.add_argument("--camera-height", type=int, default=180)
    parser.add_argument("--camera-password")
    parser.add_argument("--camera-path", default="/")
    parser.add_argument("--camera-port", type=int, default=554)
    parser.add_argument("--camera-protocols")
    parser.add_argument("--camera-rate-KB-per-sec", type=float, default=25.0, help="rate in KB/sec")
    parser.add_argument("--camera-uri")
    parser.add_argument("--camera-user")
    parser.add_argument("--camera-width", type=int, default=320)
    parser.add_argument("--debugspy", type=str2bool, default=False)
    parser.add_argument("--fakesink", type=str2bool, default=False)
    parser.add_argument("--fakesource", type=str2bool, default=False)
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope", required=True)
    parser.add_argument("--pravega-stream", required=True)
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("%s: BEGIN" % parser.prog)

    # Set default GStreamer logging.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = ("WARNING,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO," +
            "h264parse:WARN,pravegasink:LOG")

    # Set default logging for pravega-video, which sets a Rust tracing subscriber used by the Pravega Rust Client.
    if not "PRAVEGA_VIDEO_LOG" in os.environ:
        os.environ["PRAVEGA_VIDEO_LOG"] = "info"

    # Print configuration parameters.
    for arg in vars(args):
        if 'password' not in arg:
            logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    # Build camera_uri from components.
    if args.camera_uri is None:
        if args.camera_address is None:
            raise Exception("If camera-uri is empty, then camera-address is required.")
        args.camera_uri = "rtsp://%s:%d%s" % (args.camera_address, args.camera_port, args.camera_path)
    logging.info("camera_uri=%s" % args.camera_uri)

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create GStreamer pipeline.

    if args.fakesource:
        caps = "video/x-raw,format=YUY2,width=%d,height=%d,framerate=30/1" % (args.camera_width, args.camera_height)
        kbits_per_sec = int(args.camera_rate_KB_per_sec * 8.0)
        source_desc = (
            "videotestsrc name=src is-live=true do-timestamp=true\n" +
            "   ! " + caps + "\n" +
            "   ! videoconvert\n" +
            "   ! clockoverlay\n" +
            "   ! timeoverlay\n" +
            "   ! videoconvert\n" +
            "   ! queue\n" +
            "   ! x264enc tune=zerolatency key-int-max=30 bitrate=%d\n" %(kbits_per_sec))
    else:
        source_desc = (
            "rtspsrc name=rtspsrc\n" +
            # Extract H264 elementary stream
            "   ! rtph264depay\n")

    if args.fakesink:
        sink_desc = "   ! fakesink name=fakesink\n"
    else:
        sink_desc = "   ! pravegasink name=pravegasink\n"

    if args.debugspy:
        debugspy_desc = "   ! debugspy checksum-type=md5\n"
    else:
        debugspy_desc = ""

    pipeline_description = (
        source_desc +
        debugspy_desc +
        # Must align on Access Units for mpegtsmux
        "   ! h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        # Packetize in MPEG transport stream
        "   ! mpegtsmux\n" +
        "   ! tsparse alignment=21 split-on-rai=true\n"
        "   ! queue name=queue_sink\n" +
        sink_desc)
    logging.info("Creating pipeline:\n" +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    source = pipeline.get_by_name("rtspsrc")
    if source:
        source.set_property("location", args.camera_uri)
        if args.camera_user:
            source.set_property("user-id", args.camera_user)
        if args.camera_password:
            source.set_property("user-pw", args.camera_password)
        # Outgoing timestamps are calculated directly from the RTP timestamps. This mode is good for recording.
        # This will provide the RTP timestamps as PTS (and the arrival timestamps as DTS).
        # See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
        source.set_property("buffer-mode", "none")
        # Drop oldest buffers when the queue is completely filled
        source.set_property("drop-on-latency", True)
        # Set the maximum latency of the jitterbuffer (milliseconds).
        # Packets will be kept in the buffer for at most this time.
        source.set_property("latency", 2000)
        # Required to get NTP timestamps as PTS
        source.set_property("ntp-sync", True)
        # Required to get NTP timestamps as PTS
        source.set_property("ntp-time-source", "running-time")
        if args.camera_protocols:
            source.set_property("protocols", args.camera_protocols)
    clockoverlay = pipeline.get_by_name("clockoverlay0")
    if clockoverlay:
        clockoverlay.set_property("font-desc", "Sans 48px")
        clockoverlay.set_property("time-format", "%F %T")
        clockoverlay.set_property("shaded-background", True)
    timeoverlay = pipeline.get_by_name("timeoverlay0")
    if timeoverlay:
        timeoverlay.set_property("font-desc", "Sans 48px")
        timeoverlay.set_property("valignment", "bottom")
        timeoverlay.set_property("shaded-background", True)
    queue_sink = pipeline.get_by_name("queue_sink")
    if queue_sink:
        queue_sink.set_property("max-size-buffers", 0)
        queue_sink.set_property("max-size-bytes", int(args.buffer_size_mb * 1024 * 1024))
        queue_sink.set_property("max-size-time", 0)
        queue_sink.set_property("silent", True)
        queue_sink.set_property("leaky", "downstream")
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("allow-create-scope", args.allow_create_scope)
        pravegasink.set_property("controller", args.pravega_controller_uri)
        pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasink.set_property("stream", "%s/%s" % (args.pravega_scope, args.pravega_stream))
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        # Required to use NTP timestamps in PTS
        if not args.fakesource:
            pravegasink.set_property("timestamp-mode", "ntp")

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    def shutdown_handler(signum, frame):
        logging.info("%s: Received signal %s" % (parser.prog, signum))
        pipeline.set_state(Gst.State.NULL)

    signal.signal(signal.SIGINT, shutdown_handler)
    signal.signal(signal.SIGTERM, shutdown_handler)

    # Start play back and listen to events.
    logging.info("Starting pipeline")
    pipeline.set_state(Gst.State.PLAYING)
    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        pipeline.set_state(Gst.State.NULL)
        raise

    pipeline.set_state(Gst.State.NULL)

    logging.info("%s: END" % parser.prog)

if __name__ == "__main__":
    main()
