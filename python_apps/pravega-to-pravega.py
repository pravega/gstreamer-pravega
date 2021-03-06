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
# Copy a Pravega stream written by pravegasink to another stream.
#

import configargparse as argparse
import datetime
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst

from gstpravega import HealthCheckServer, bus_call, make_element, resolve_pravega_stream, str2bool


def main():
    parser = argparse.ArgumentParser(
        description="Copy a Pravega stream written by pravegasink to another stream",
        auto_env_var_prefix="")
    parser.add_argument("--allow-create-scope", type=str2bool, default=True)
    parser.add_argument("--input-stream", required=True, metavar="SCOPE/STREAM")
    parser.add_argument("--gst-debug",
        default="WARNING,pravegasrc:DEBUG,pravegatc:DEBUG,pravegasink:DEBUG")
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope")
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--rust-log", default="warn")
    parser.add_argument("--output-stream", required=True,
        help="Name of output stream.", metavar="SCOPE/STREAM")
    parser.add_argument("--recovery-table", metavar="SCOPE/TABLE")
    parser.add_argument("--sleep-sec", type=float, default=0.0, help="Delay pipeline start by this many seconds")
    parser.add_argument("--start-mode", default="earliest")
    parser.add_argument("--start-utc")
    HealthCheckServer.add_arguments(parser)
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    args.input_stream = resolve_pravega_stream(args.input_stream, args.pravega_scope)
    args.output_stream = resolve_pravega_stream(args.output_stream, args.pravega_scope)
    args.recovery_table = resolve_pravega_stream(args.recovery_table, args.pravega_scope)

    # Print configuration parameters.
    for arg in vars(args):
        logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    # Set GStreamer log level.
    os.environ["GST_DEBUG"] = args.gst_debug
    # Initialize a Rust tracing subscriber which is used by the Pravega Rust Client in pravegasrc, pravegasink, and libnvds_pravega_proto.
    # Either of these environment variables may be used, depending on the load order.
    os.environ["PRAVEGA_VIDEO_LOG"] = args.rust_log

    health_check_server = HealthCheckServer(**vars(args))

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())
    loop = GObject.MainLoop()

    if args.recovery_table:
        pravegatc_pipeline = "   ! pravegatc name=pravegatc\n"
    else:
        pravegatc_pipeline = ""

    pipeline_desc = (
        "pravegasrc name=pravegasrc\n" +
        pravegatc_pipeline +
        "   ! identity name=to_pravegasink silent=false\n" +
        "   ! pravegasink name=pravegasink\n" +
        "")

    logging.info("Creating pipeline:\n" +  pipeline_desc)
    pipeline = Gst.parse_launch(pipeline_desc)

    # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
    pipeline.add_property_deep_notify_watch(None, True)

    pravegasrc = pipeline.get_by_name("pravegasrc")
    pravegasrc.set_property("controller", args.pravega_controller_uri)
    pravegasrc.set_property("stream", args.input_stream)
    pravegasrc.set_property("allow-create-scope", args.allow_create_scope)
    pravegasrc.set_property("keycloak-file", args.keycloak_service_account_file)
    pravegasrc.set_property("start-mode", args.start_mode)
    if args.start_utc:
        pravegasrc.set_property("start-utc", args.start_utc)
    pravegatc = pipeline.get_by_name("pravegatc")
    if pravegatc:
        pravegatc.set_property("controller", args.pravega_controller_uri)
        pravegatc.set_property("table", args.recovery_table)
        pravegatc.set_property("keycloak-file", args.keycloak_service_account_file)
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("allow-create-scope", args.allow_create_scope)
        pravegasink.set_property("controller", args.pravega_controller_uri)
        if args.keycloak_service_account_file:
            pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasink.set_property("stream", args.output_stream)
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        pravegasink.set_property("timestamp-mode", "tai")
    health_check_server.add_probe(pipeline, "pravegasink", "sink")

    # Feed GStreamer bus messages to event loop.
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    if args.sleep_sec > 0.0:
        logging.info("Sleeping for %f seconds" % args.sleep_sec)
        time.sleep(args.sleep_sec)

    # Start pipelines.
    logging.info("Starting pipelines")
    pipeline.set_state(Gst.State.PLAYING)

    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        pipeline.set_state(Gst.State.NULL)
        raise

    pipeline.set_state(Gst.State.NULL)
    logging.info("END")


if __name__ == "__main__":
    main()
