#!/usr/bin/env python3

import argparse
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst

common_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.append(common_dir)
from common.is_aarch_64 import is_aarch64
from common.bus_call import bus_call

import pyds


PGIE_CLASS_ID_VEHICLE = 0
PGIE_CLASS_ID_BICYCLE = 1
PGIE_CLASS_ID_PERSON = 2
PGIE_CLASS_ID_ROADSIGN = 3


def make_element(factory_name, element_name):
    logging.info("Creating element %s of type %s" % (element_name, factory_name))
    element = Gst.ElementFactory.make(factory_name, element_name)
    if not element:
        raise Exception("Unable to create element %s of type %s" % (element_name, factory_name))
    return element


def format_ns(ns):
    s, ns = divmod(ns, 1000000000)
    m, s = divmod(s, 60)
    h, m = divmod(m, 60)
    return "%u:%02u:%02u.%09u" % (h, m, s, ns)


def osd_sink_pad_buffer_probe(pad, info, user_data):
    frame_number=0
    # Intiallizing object counter with 0.
    obj_counter = {
        PGIE_CLASS_ID_VEHICLE:0,
        PGIE_CLASS_ID_PERSON:0,
        PGIE_CLASS_ID_BICYCLE:0,
        PGIE_CLASS_ID_ROADSIGN:0
    }
    num_rects=0

    gst_buffer = info.get_buffer()
    if not gst_buffer:
        logging.error("Unable to get GstBuffer")
        return

    # Retrieve batch metadata from the gst_buffer
    # Note that pyds.gst_buffer_get_nvds_batch_meta() expects the
    # C address of gst_buffer as input, which is obtained with hash(gst_buffer)
    batch_meta = pyds.gst_buffer_get_nvds_batch_meta(hash(gst_buffer))
    l_frame = batch_meta.frame_meta_list
    while l_frame is not None:
        try:
            # Note that l_frame.data needs a cast to pyds.NvDsFrameMeta
            # The casting is done by pyds.NvDsFrameMeta.cast()
            # The casting also keeps ownership of the underlying memory
            # in the C code, so the Python garbage collector will leave
            # it alone.
            frame_meta = pyds.NvDsFrameMeta.cast(l_frame.data)
        except StopIteration:
            break

        frame_number=frame_meta.frame_num
        num_rects = frame_meta.num_obj_meta
        l_obj=frame_meta.obj_meta_list
        while l_obj is not None:
            try:
                # Casting l_obj.data to pyds.NvDsObjectMeta
                obj_meta=pyds.NvDsObjectMeta.cast(l_obj.data)
            except StopIteration:
                break
            obj_counter[obj_meta.class_id] += 1
            try:
                l_obj=l_obj.next
            except StopIteration:
                break

        # Acquiring a display meta object. The memory ownership remains in
        # the C code so downstream plugins can still access it. Otherwise
        # the garbage collector will claim it when this probe function exits.
        display_meta=pyds.nvds_acquire_display_meta_from_pool(batch_meta)
        display_meta.num_labels = 1
        py_nvosd_text_params = display_meta.text_params[0]
        # Setting display text to be shown on screen
        # Note that the pyds module allocates a buffer for the string, and the
        # memory will not be claimed by the garbage collector.
        # Reading the display_text field here will return the C address of the
        # allocated string. Use pyds.get_string() to get the string content.
        py_nvosd_text_params.display_text = "Frame Number={} Number of Objects={} Vehicle_count={} Person_count={}".format(frame_number, num_rects, obj_counter[PGIE_CLASS_ID_VEHICLE], obj_counter[PGIE_CLASS_ID_PERSON])

        # Now set the offsets where the string should appear
        py_nvosd_text_params.x_offset = 10
        py_nvosd_text_params.y_offset = 12

        # Font , font-color and font-size
        py_nvosd_text_params.font_params.font_name = "Serif"
        py_nvosd_text_params.font_params.font_size = 10
        # set(red, green, blue, alpha); set to White
        py_nvosd_text_params.font_params.font_color.set(1.0, 1.0, 1.0, 1.0)

        # Text background color
        py_nvosd_text_params.set_bg_clr = 1
        # set(red, green, blue, alpha); set to Black
        py_nvosd_text_params.text_bg_clr.set(0.0, 0.0, 0.0, 1.0)
        # Using pyds.get_string() to get display_text as string
        logging.info(pyds.get_string(py_nvosd_text_params.display_text))
        pyds.nvds_add_display_meta_to_frame(frame_meta, display_meta)
        try:
            l_frame=l_frame.next
        except StopIteration:
            break

    return Gst.PadProbeReturn.OK


def test_pad_buffer_probe(pad, info, user_data):
    # logging.info("test_pad_buffer_probe")
    # logging.info("test_pad_buffer_probe: pad=%s, info=%s, user_data=%s" % (str(pad), str(info), str(user_data)))
    gst_buffer = info.get_buffer()
    # logging.info("test_pad_buffer_probe: gst_buffer=%s" % (str(gst_buffer),))
    if not gst_buffer:
        logging.error("Unable to get GstBuffer")
        return
    pts = gst_buffer.pts
    logging.info("test_pad_buffer_probe: pts=%s" % (format_ns(pts),))
    return Gst.PadProbeReturn.OK


def main():
    parser = argparse.ArgumentParser(description="Capture from RTSP camera, detect objects, display on screen")
    parser.add_argument("--log_level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--pgie_config_file",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "pgie_config.txt"))
    parser.add_argument("--source-uri", required=True)
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    # Set GStreamer log level.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = "WARNING,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,h264parse:LOG,nvv4l2decoder:LOG"

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    pipeline_description = (
        "rtspsrc name=source ! queue name=queue_probe ! rtph264depay ! queue name=queue_probe3 " +
        "! nvv4l2decoder name=decoder ! queue " +
        "! streammux.sink_0 nvstreammux name=streammux ! queue ! nvinfer name=pgie ! queue name=queue_probe2 " +
        "! nvvideoconvert ! queue ! nvdsosd name=nvosd ! queue ! nveglglessink name=sink"
    )
    logging.info("Creating pipeline: " +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    source = pipeline.get_by_name("source")
    source.set_property("location", args.source_uri)
    source.set_property("buffer-mode", "none")
    # source.set_property("drop-messages-interval", 0)
    source.set_property("drop-on-latency", True)
    source.set_property("latency", 2000)
    source.set_property("ntp-sync", True)
    source.set_property("ntp-time-source", "running-time")
    # source.set_property("rtcp-sync-send-time", False)
    streammux = pipeline.get_by_name("streammux")
    streammux.set_property("width", 1920)
    streammux.set_property("height", 1080)
    streammux.set_property("batch-size", 1)
    streammux.set_property("batched-push-timeout", 4000000)
    streammux.set_property("live-source", 1)
    pgie = pipeline.get_by_name("pgie")
    pgie.set_property("config-file-path", args.pgie_config_file)

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    # Lets add probe to get informed of the meta data generated, we add probe to
    # the sink pad of the osd element, since by that time, the buffer would have
    # had got all the metadata.
    nvosd = pipeline.get_by_name("nvosd")
    osd_sinkpad = nvosd.get_static_pad("sink")
    if not osd_sinkpad:
        raise Exception("Unable to get sink pad of nvosd")
    osd_sinkpad.add_probe(Gst.PadProbeType.BUFFER, osd_sink_pad_buffer_probe, 0)

    queue_probe = pipeline.get_by_name("queue_probe")
    queue_probe_sinkpad = queue_probe.get_static_pad("sink")
    if not queue_probe_sinkpad:
        raise Exception("Unable to get sink pad of queue_probe")
    queue_probe_sinkpad.add_probe(Gst.PadProbeType.BUFFER, test_pad_buffer_probe, 0)

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
    logging.info("END")


if __name__ == "__main__":
    main()
