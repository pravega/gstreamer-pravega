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
# Import a video file to a Pravega stream.
#

import configargparse as argparse
import ctypes
import datetime
import distutils.util
import logging
import os
import signal
import sys
import time
import traceback
from gstpravega import bus_call


import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


def str2bool(v):
    return bool(distutils.util.strtobool(v))


def main():
    parser = argparse.ArgumentParser(
        description="Import a video file to a Pravega stream. This transcodes the video to an appropriate format.",
        auto_env_var_prefix="")
    # Note that below arguments can be passed through the environment such as PRAVEGA_CONTROLLER_URI.
    parser.add_argument("--bitrate-kilobytes-per-sec", type=float, default=500)
    parser.add_argument("--fragment-duration-ms", type=int, default=1)
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--nvvideoconvert", type=str2bool, default=False)
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope", required=True)
    parser.add_argument("--pravega-stream", required=True)
    parser.add_argument("--source-uri", required=True,
                        help="URI of the file to import")
    parser.add_argument("--start-utc", required=True,
                        help="The first frame in the video file will be recorded with this timestamp in RFC 3339 format (2021-12-28T23:41:45.691Z).")
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level, format="%(asctime)s %(levelname)-7s %(message)s")
    logging.Formatter.formatTime = (lambda self, record, datefmt: datetime.datetime.fromtimestamp(record.created, datetime.timezone.utc).astimezone().isoformat())
    logging.info(f"{parser.prog}: BEGIN")

    # Set default GStreamer logging.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = ("WARNING,timestampcvt:INFO,pravegasink:DEBUG")

    # Set default logging for pravega-video, which sets a Rust tracing subscriber used by the Pravega Rust Client.
    if not "PRAVEGA_VIDEO_LOG" in os.environ:
        os.environ["PRAVEGA_VIDEO_LOG"] = "info"

    # Print configuration parameters.
    for arg in vars(args):
        logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create GStreamer pipeline.

    pipelines = []

    if args.nvvideoconvert:
        nvvideoconvert_pipeline = f"   ! nvvideoconvert\n"
    else:
        nvvideoconvert_pipeline = ""

    pipeline_description = (
        f"uridecodebin name=src\n" +
        f"   ! queue\n" +
        f"   ! timestampcvt name=timestampcvt\n" +
        f"   ! videoconvert\n" +
        nvvideoconvert_pipeline +
        f"   ! queue\n" +
        f"   ! x264enc name=x264enc\n"
        f"   ! queue\n" +
        f"   ! h264parse\n" +
        f"   ! mp4mux name=mp4mux\n" +
        f"   ! fragmp4pay\n" +
        f"   ! pravegasink name=pravegasink\n"
    )
    logging.info("Creating pipeline:\n" +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)
    pipelines += [pipeline]

    # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
    pipeline.add_property_deep_notify_watch(None, True)

    src = pipeline.get_by_name("src")
    if src:
        src.set_property("uri", args.source_uri)
    timestampcvt = pipeline.get_by_name("timestampcvt")
    if timestampcvt:
        timestampcvt.set_property("input-timestamp-mode", "start-at-fixed-time")
        timestampcvt.set_property("start-utc", args.start_utc)
    x264enc = pipeline.get_by_name("x264enc")
    if x264enc:
        x264enc.set_property("bitrate", int(args.bitrate_kilobytes_per_sec * 8))
        x264enc.set_property("key-int-max", 30)
        x264enc.set_property("tune", "zerolatency")
    mp4mux = pipeline.get_by_name("mp4mux")
    if mp4mux:
        mp4mux.set_property("streamable", True)
        mp4mux.set_property("fragment-duration", args.fragment_duration_ms)
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("controller", args.pravega_controller_uri)
        if args.keycloak_service_account_file:
            pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasink.set_property("stream", "%s/%s" % (args.pravega_scope, args.pravega_stream))
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        pravegasink.set_property("timestamp-mode", "tai")

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    def shutdown_handler(signum, frame):
        logging.info("Shutting down due to received signal %s" % signum)
        loop.quit()

    signal.signal(signal.SIGINT, shutdown_handler)
    signal.signal(signal.SIGTERM, shutdown_handler)

    # Start play back and listen to events.
    logging.info("Starting pipelines")
    for p in pipelines: p.set_state(Gst.State.PLAYING)
    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        pipeline.set_state(Gst.State.NULL)
        raise

    logging.info("Stopping pipelines")
    for p in pipelines: p.set_state(Gst.State.NULL)
    logging.info(f"{parser.prog}: END")


if __name__ == "__main__":
    main()
