#!/usr/bin/env python3
#
# Experiment with writing video to Pravega, then reading it, to confirm that timestamps match.
#
# TODO: This results in buffers from tsmux having PTS exactly 125 ms greater than the input into mpegtsmux. Why?
#

import argparse
import ctypes
import logging
import os
import sys
import time
import traceback
import uuid

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


def bus_call(bus, message, loop):
    """Callback for GStreamer bus messages"""
    t = message.type
    if t == Gst.MessageType.EOS:
        logging.info('End-of-stream')
        loop.quit()
    elif t == Gst.MessageType.WARNING:
        err, debug = message.parse_warning()
        logging.warn('%s: %s' % (err, debug))
    elif t == Gst.MessageType.ERROR:
        err, debug = message.parse_error()
        logging.error('%s: %s' % (err, debug))
        loop.quit()
    return True


def make_element(factory_name, element_name):
    """Create a GStreamer element, raising an exception on failure."""
    logging.info("Creating element %s of type %s" % (element_name, factory_name))
    element = Gst.ElementFactory.make(factory_name, element_name)
    if not element:
        raise Exception("Unable to create element %s of type %s" % (element_name, factory_name))
    return element


def format_clock_time(ns):
    """Format time in nanoseconds like 01:45:35.975000000"""
    s, ns = divmod(ns, 1000000000)
    m, s = divmod(s, 60)
    h, m = divmod(m, 60)
    return "%u:%02u:%02u.%09u" % (h, m, s, ns)


def show_metadata_probe(pad, info, user_data):
    """Buffer probe to show metadata in a buffer"""
    gst_buffer = info.get_buffer()
    if gst_buffer:
        logging.info("show_metadata_probe: %20s:%-8s: pts=%23s, dts=%23s, duration=%23s, size=%8d" %
            (pad.get_parent_element().name,
            pad.name, 
            format_clock_time(gst_buffer.pts),
            format_clock_time(gst_buffer.dts),
            format_clock_time(gst_buffer.duration),
            gst_buffer.get_size()))
    else:
        logging.info("show_metadata_probe: %20s:%-8s: no buffer")
    return Gst.PadProbeReturn.OK


def add_probe(pipeline, element_name, callback, pad_name="sink", probe_type=Gst.PadProbeType.BUFFER):
    element = pipeline.get_by_name(element_name)
    if not element:
        raise Exception("Unable to get element %s" % element_name)
    sinkpad = element.get_static_pad(pad_name)
    if not sinkpad:
        raise Exception("Unable to get %s pad of %s" % (pad_name, element_name))
    sinkpad.add_probe(probe_type, callback, 0)


def main():
    parser = argparse.ArgumentParser(
        description="")
    parser.add_argument("--controller", default="192.168.1.123:9090")
    parser.add_argument("--log_level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--stream", default="examples/%s" % uuid.uuid4())
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    # Set GStreamer plugin path.
    script_dir = os.path.dirname(os.path.abspath(__file__))
    python_plugin_dir = os.path.join(script_dir, 'plugins')
    logging.info('python_plugin_dir=%s' % python_plugin_dir)
    plugin_path = ':'.join([python_plugin_dir, os.environ['GST_PLUGIN_PATH']])
    logging.debug('plugin_path=%s' % plugin_path)
    os.environ['GST_PLUGIN_PATH'] = plugin_path

    # Set GStreamer log level.
    if not "GST_DEBUG" in os.environ:
        # GST_DEBUG log level can be: ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
        os.environ["GST_DEBUG"] = ("INFO,pravegasink:LOG,pravegasrc:LOG," +
            "mpegtsmux:TRACE,mpegtsbase:TRACE,basetsmux:TRACE,mpegtspacketizer:TRACE,tsparse:TRACE,tsdemux:LOG,h264parse:LOG")
    if not "PRAVEGA_VIDEO_LOG" in os.environ:
        # PRAVEGA_VIDEO_LOG log level can be: error, warn, info, debug, trace
        os.environ["PRAVEGA_VIDEO_LOG"] = "info"

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())
    loop = GObject.MainLoop()
    pipelines = []

    #
    # Write + read pipeline (without Pravega)
    #

    if False:
        pipeline_description = (
            "videotestsrc is-live=true num-buffers=90\n" +
            "   ! video/x-raw,format=YUY2,width=320,height=180,framerate=30/1\n" +
            "   ! live_feed_simulator name=live_feed_simulator\n" +
            "   ! videoconvert\n" +
            "   ! x264enc tune=zerolatency key-int-max=30 bitrate=200\n" +            
            "   ! mpegtsmux\n" +
            "   ! identity name=id_from_mpegtsmux\n" +
            # "   ! tsparse name=tsparse ignore-pcr=true\n" +
            "   ! tsdemux name=tsdemux ignore-pcr=false\n" +
            "   ! h264parse name=h264parse\n" +
            "   ! video/x-h264,alignment=au\n" +
            "   ! avdec_h264 name=avdec_h264\n" +
            # "   ! fakesink\n" +
            "   ! autovideosink sync=false\n"
            "")
        logging.info("Creating pipeline:\n" +  pipeline_description)
        pipeline = Gst.parse_launch(pipeline_description)

        add_probe(pipeline, "videotestsrc0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "videoconvert0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "live_feed_simulator", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "x264enc0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "id_from_mpegtsmux", show_metadata_probe, pad_name='src')
        # add_probe(pipeline, "tsparse", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "h264parse", show_metadata_probe, pad_name='sink')
        add_probe(pipeline, "h264parse", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "avdec_h264", show_metadata_probe, pad_name='src')

        pipelines += [pipeline]

    #
    # Write pipeline
    #

    if True:
        pipeline_description = (
            "videotestsrc is-live=true num-buffers=90\n" +
            "   ! video/x-raw,format=YUY2,width=320,height=180,framerate=30/1\n" +
            "   ! live_feed_simulator name=live_feed_simulator\n" +
            "   ! videoconvert\n" +
            "   ! x264enc tune=zerolatency key-int-max=30 bitrate=200\n" +            
            "   ! mpegtsmux name=mpegtsmux0\n" +
            # "   ! identity name=id_from_mpegtsmux\n" +
            # "   ! tsparse name=tsparse0\n" +
            # "   ! queue\n" +
            "   ! pravegasink timestamp-mode=ntp sync=false\n" +
            # "   ! decodebin ! autovideosink\n" +
            # "   ! fakesink\n" +
            "")
        logging.info("Creating pipeline:\n" +  pipeline_description)
        pipeline = Gst.parse_launch(pipeline_description)

        pravegasink = pipeline.get_by_name("pravegasink0")
        if pravegasink:
            pravegasink.set_property("controller", args.controller)
            pravegasink.set_property("stream", args.stream)

        add_probe(pipeline, "videotestsrc0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "videoconvert0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "live_feed_simulator", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "x264enc0", show_metadata_probe, pad_name='src')
        # add_probe(pipeline, "mpegtsmux0", show_metadata_probe, pad_name='sink_0')
        add_probe(pipeline, "mpegtsmux0", show_metadata_probe, pad_name='src')
        # add_probe(pipeline, "id_from_mpegtsmux", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "pravegasink0", show_metadata_probe, pad_name='sink')
        # add_probe(pipeline, "fakesink0", show_metadata_probe, pad_name='sink')

        pipelines += [pipeline]

    #
    # Read pipeline
    #

    if True:
        pipeline_description = (
            "pravegasrc\n" +
            "   ! tsdemux name=tsdemux latency=0\n" +
            "   ! h264parse name=h264parse\n" +
            "   ! video/x-h264,alignment=au\n" +
            "   ! avdec_h264 name=avdec_h264\n" +
            "   ! fakesink\n" +
            "")
        logging.info("Creating pipeline:\n" +  pipeline_description)
        pipeline = Gst.parse_launch(pipeline_description)

        pravegasrc = pipeline.get_by_name("pravegasrc0")
        if pravegasrc:
            pravegasrc.set_property("controller", args.controller)
            pravegasrc.set_property("stream", args.stream)

        add_probe(pipeline, "pravegasrc0", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "h264parse", show_metadata_probe, pad_name='sink')
        add_probe(pipeline, "h264parse", show_metadata_probe, pad_name='src')
        add_probe(pipeline, "avdec_h264", show_metadata_probe, pad_name='src')

        pipelines += [pipeline]

    #
    # Start pipelines.
    #

    for pipeline in pipelines:
        # Feed GStreamer bus messages to event loop.
        bus = pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", bus_call, loop)

    logging.info("Starting pipelines")
    for p in pipelines: p.set_state(Gst.State.PLAYING)

    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        for p in pipelines: p.set_state(Gst.State.NULL)
        raise

    for p in pipelines: p.set_state(Gst.State.NULL)
    logging.info("END")


if __name__ == "__main__":
    main()
