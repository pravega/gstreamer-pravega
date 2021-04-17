#!/usr/bin/env python3
#
# Capture from RTSP camera, detect objects, write video data and metadata to a Pravega stream.
# Note: This method has some drawbacks. See pravega-to-object-detection-to-pravega.py for the recommended method.
# Drawbacks:
#   - RTSP camera recording is lost when the inference model must get updated, or this process is interrupted
#     for any other reason.
#   - For unknown reasons, timestamps are off 125 ms in a roundtrip from mpegtsmux to tsdemux.
#     Using this method makes metadata and data timestamps inconsistent.
#

import argparse
import ctypes
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst

# See https://docs.nvidia.com/metropolis/deepstream/5.0DP/python-api/
import pyds


MAX_TIME_STAMP_LEN = 32
PGIE_CLASS_ID_VEHICLE = 0
PGIE_CLASS_ID_BICYCLE = 1
PGIE_CLASS_ID_PERSON = 2
PGIE_CLASS_ID_ROADSIGN = 3


def long_to_int(l):
    value = ctypes.c_int(l & 0xffffffff).value
    return value


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


def glist_iterator(li):
    """Iterator for a pyds.GLib object"""
    while li is not None:
        yield li.data
        li = li.next


# See https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_metadata.html
def show_metadata_probe(pad, info, user_data):
    """Buffer probe to show metadata in a buffer"""
    gst_buffer = info.get_buffer()
    if gst_buffer:
        batch_meta = pyds.gst_buffer_get_nvds_batch_meta(hash(gst_buffer))
        if batch_meta:
            for frame_meta_raw in glist_iterator(batch_meta.frame_meta_list):
                frame_meta = pyds.NvDsFrameMeta.cast(frame_meta_raw)
                logging.info("show_metadata_probe: pad=%s:%s, pts=%s, buf_pts=%s, ntp_timestamp=%s" %
                    (pad.get_parent_element().name, pad.name, format_clock_time(gst_buffer.pts), format_clock_time(frame_meta.buf_pts), str(frame_meta.ntp_timestamp)))
                for obj_meta_raw in glist_iterator(frame_meta.obj_meta_list):
                    obj_meta = pyds.NvDsObjectMeta.cast(obj_meta_raw)
                    logging.info("show_metadata_probe: obj_meta.class_id=%d" % (obj_meta.class_id,))
                for user_meta_raw in glist_iterator(frame_meta.frame_user_meta_list):
                    user_meta = pyds.NvDsUserMeta.cast(user_meta_raw)
                    logging.info("show_metadata_probe: user_meta=%s" % (str(user_meta),))
    return Gst.PadProbeReturn.OK


# Callback function for deep-copying an NvDsEventMsgMeta struct
def meta_copy_func(data, user_data):
    logging.debug("meta_copy_func: BEGIN")
    # Cast data to pyds.NvDsUserMeta
    user_meta = pyds.NvDsUserMeta.cast(data)
    src_meta_data = user_meta.user_meta_data
    # Cast src_meta_data to pyds.NvDsEventMsgMeta
    srcmeta = pyds.NvDsEventMsgMeta.cast(src_meta_data)
    # Duplicate the memory contents of srcmeta to dstmeta
    # First use pyds.get_ptr() to get the C address of srcmeta, then
    # use pyds.memdup() to allocate dstmeta and copy srcmeta into it.
    # pyds.memdup returns C address of the allocated duplicate.
    dstmeta_ptr = pyds.memdup(pyds.get_ptr(srcmeta), sys.getsizeof(pyds.NvDsEventMsgMeta))
    # Cast the duplicated memory to pyds.NvDsEventMsgMeta
    dstmeta = pyds.NvDsEventMsgMeta.cast(dstmeta_ptr)

    # Duplicate contents of ts field. Note that reading srcmeta.ts
    # returns its C address. This allows to memory operations to be
    # performed on it.
    dstmeta.ts = pyds.memdup(srcmeta.ts, MAX_TIME_STAMP_LEN + 1)

    # Copy the sensorStr. This field is a string property.
    # The getter (read) returns its C address. The setter (write)
    # takes string as input, allocates a string buffer and copies
    # the input string into it.
    # pyds.get_string() takes C address of a string and returns
    # the reference to a string object and the assignment inside the binder copies content.
    dstmeta.sensorStr = pyds.get_string(srcmeta.sensorStr)

    if srcmeta.objSignature.size > 0:
        dstmeta.objSignature.signature = pyds.memdup(srcmeta.objSignature.signature, srcMeta.objSignature.size)
        dstmeta.objSignature.size = srcmeta.objSignature.size

    if srcmeta.extMsgSize > 0:
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE:
            srcobj = pyds.NvDsVehicleObject.cast(srcmeta.extMsg)
            obj = pyds.alloc_nvds_vehicle_object()
            obj.type = pyds.get_string(srcobj.type)
            obj.make = pyds.get_string(srcobj.make)
            obj.model = pyds.get_string(srcobj.model)
            obj.color = pyds.get_string(srcobj.color)
            obj.license = pyds.get_string(srcobj.license)
            obj.region = pyds.get_string(srcobj.region)
            dstmeta.extMsg = obj
            dstmeta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)
        elif srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON:
            srcobj = pyds.NvDsPersonObject.cast(srcmeta.extMsg)
            obj = pyds.alloc_nvds_person_object()
            obj.age = srcobj.age
            obj.gender = pyds.get_string(srcobj.gender)
            obj.cap = pyds.get_string(srcobj.cap)
            obj.hair = pyds.get_string(srcobj.hair)
            obj.apparel = pyds.get_string(srcobj.apparel)
            dstmeta.extMsg = obj
            dstmeta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)

    logging.debug("meta_copy_func: END")
    return dstmeta


# Callback function for freeing an NvDsEventMsgMeta instance
def meta_free_func(data, user_data):
    logging.debug("meta_free_func: BEGIN")
    user_meta = pyds.NvDsUserMeta.cast(data)
    srcmeta = pyds.NvDsEventMsgMeta.cast(user_meta.user_meta_data)

    # pyds.free_buffer takes C address of a buffer and frees the memory
    # It's a NOP if the address is NULL
    pyds.free_buffer(srcmeta.ts)
    pyds.free_buffer(srcmeta.sensorStr)

    if srcmeta.objSignature.size > 0:
        pyds.free_buffer(srcmeta.objSignature.signature)
        srcmeta.objSignature.size = 0

    if srcmeta.extMsgSize > 0:
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE:
            obj = pyds.NvDsVehicleObject.cast(srcmeta.extMsg)
            pyds.free_buffer(obj.type)
            pyds.free_buffer(obj.color)
            pyds.free_buffer(obj.make)
            pyds.free_buffer(obj.model)
            pyds.free_buffer(obj.license)
            pyds.free_buffer(obj.region)
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON:
            obj = pyds.NvDsPersonObject.cast(srcmeta.extMsg)
            pyds.free_buffer(obj.gender)
            pyds.free_buffer(obj.cap)
            pyds.free_buffer(obj.hair)
            pyds.free_buffer(obj.apparel)
        pyds.free_gbuffer(srcmeta.extMsg)
        srcmeta.extMsgSize = 0
    logging.debug("meta_free_func: END")


def generate_vehicle_meta(data):
    obj = pyds.NvDsVehicleObject.cast(data)
    obj.type = "sedan"
    obj.color = "blue"
    obj.make = "Bugatti"
    obj.model = "M"
    obj.license = "XX1234"
    obj.region = "CA"
    return obj


def generate_person_meta(data):
    obj = pyds.NvDsPersonObject.cast(data)
    obj.age = 45
    obj.cap = "none"
    obj.hair = "black"
    obj.gender = "male"
    obj.apparel= "formal"
    return obj


def create_pyds_string(s):
    """Create a C zero-terminated string from the provided Python string.
    The caller is responsible for freeing the string."""
    sb = ctypes.create_string_buffer(s.encode("utf-8"))
    return pyds.strdup(ctypes.addressof(sb))


# TODO: Replace with PravegaTimestamp class in pravega-to-object-detection-to-pravega.py.
def pts_to_pravega_video_timestamp(pts):
    """Convert a pts (nanoseconds since the NTP epoch 1900-01-01 00:00:00 UTC, minus leap seconds)
    to the timestamp used by pravegasink in gst-plugin-pravega (nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI).
    If the time cannot be represented, 0 will be returned.
    This must match PravegaTimestamp::from_ntp_nanoseconds() in pravega-video/src/timestamp.rs."""
    # Difference between NTP and Unix epochs.
    # Equals 70 years plus 17 leap days.
    # See [https://stackoverflow.com/a/29138806/5890553].
    UNIX_TO_NTP_SECONDS = (70 * 365 + 17) * 24 * 60 * 60
    # UTC to TAI offset.
    # Below is valid for dates between 2017-01-01 and the next leap second.
    # TODO: Beyond this range, we must use a table that incorporates the leap second schedule.
    # See [https://en.wikipedia.org/wiki/International_Atomic_Time].
    UTC_TO_TAI_SECONDS = 37
    ts = max(0, pts + (UTC_TO_TAI_SECONDS - UNIX_TO_NTP_SECONDS) * 1000*1000*1000)
    return ts


def generate_event_msg_meta(data, class_id, timestamp):
    logging.debug("generate_event_msg_meta: BEGIN")
    meta = pyds.NvDsEventMsgMeta.cast(data)
    meta.sensorId = 0
    meta.placeId = 0
    meta.moduleId = 0
    meta.sensorStr = "sensor-0"
    meta.ts = create_pyds_string(str(timestamp))

    # This demonstrates how to attach custom objects.
    # Any custom object as per requirement can be generated and attached
    # like NvDsVehicleObject / NvDsPersonObject. Then that object should
    # be handled in payload generator library (nvmsgconv.cpp) accordingly.
    if class_id == PGIE_CLASS_ID_VEHICLE:
        meta.type = pyds.NvDsEventType.NVDS_EVENT_MOVING
        meta.objType = pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE
        meta.objClassId = PGIE_CLASS_ID_VEHICLE
        obj = pyds.alloc_nvds_vehicle_object()
        obj = generate_vehicle_meta(obj)
        meta.extMsg = obj
        meta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)
    elif class_id == PGIE_CLASS_ID_PERSON:
        meta.type = pyds.NvDsEventType.NVDS_EVENT_ENTRY
        meta.objType = pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON
        meta.objClassId = PGIE_CLASS_ID_PERSON
        obj = pyds.alloc_nvds_person_object()
        obj = generate_person_meta(obj)
        meta.extMsg = obj
        meta.extMsgSize = sys.getsizeof(pyds.NvDsPersonObject)
    logging.debug("generate_event_msg_meta: END")
    return meta


def set_event_message_meta_probe(pad, info, u_data):
    logging.info("set_event_message_meta_probe: BEGIN")
    gst_buffer = info.get_buffer()
    if gst_buffer:
        batch_meta = pyds.gst_buffer_get_nvds_batch_meta(hash(gst_buffer))
        if batch_meta:
            for frame_meta_raw in glist_iterator(batch_meta.frame_meta_list):
                frame_meta = pyds.NvDsFrameMeta.cast(frame_meta_raw)
                # TODO: It appears that the timestamp may be incorrect by up to 1 second.
                pravega_video_timestamp = pts_to_pravega_video_timestamp(frame_meta.buf_pts)
                logging.info("set_event_message_meta_probe: pad=%s:%s, pts=%s, buf_pts=%s, pravega_video_timestamp=%d, ntp_timestamp=%s" %
                    (pad.get_parent_element().name, pad.name, format_clock_time(gst_buffer.pts), 
                    format_clock_time(frame_meta.buf_pts), pravega_video_timestamp, str(frame_meta.ntp_timestamp)))
                if pravega_video_timestamp <= 0:
                    logging.info("set_event_message_meta_probe: Timestamp is invalid. It may take a few seconds for RTSP timestamps to be valid.")
                else:
                    is_first_object = True
                    for obj_meta_raw in glist_iterator(frame_meta.obj_meta_list):
                        obj_meta = pyds.NvDsObjectMeta.cast(obj_meta_raw)
                        logging.info("set_event_message_meta_probe: obj_meta.class_id=%d" % (obj_meta.class_id,))
                        # We can only identify a single object in an NvDsEventMsgMeta.
                        # For now, we identify the first object in the frame.
                        # TODO: Create multiple NvDsEventMsgMeta instances per frame or use a custom user metadata class to identify multiple objects.
                        if is_first_object:
                            # Allocating an NvDsEventMsgMeta instance and getting reference
                            # to it. The underlying memory is not manged by Python so that
                            # downstream plugins can access it. Otherwise the garbage collector
                            # will free it when this probe exits.
                            msg_meta = pyds.alloc_nvds_event_msg_meta()
                            msg_meta.bbox.top = obj_meta.rect_params.top
                            msg_meta.bbox.left = obj_meta.rect_params.left
                            msg_meta.bbox.width = obj_meta.rect_params.width
                            msg_meta.bbox.height = obj_meta.rect_params.height
                            msg_meta.frameId = frame_meta.frame_num
                            msg_meta.trackingId = long_to_int(obj_meta.object_id)
                            msg_meta.confidence = obj_meta.confidence
                            msg_meta = generate_event_msg_meta(msg_meta, obj_meta.class_id, pravega_video_timestamp)
                            user_event_meta = pyds.nvds_acquire_user_meta_from_pool(batch_meta)
                            if user_event_meta:
                                user_event_meta.user_meta_data = msg_meta
                                user_event_meta.base_meta.meta_type = pyds.NvDsMetaType.NVDS_EVENT_MSG_META
                                # Setting callbacks in the event msg meta. The bindings layer
                                # will wrap these callables in C functions. Currently only one
                                # set of callbacks is supported.
                                pyds.set_user_copyfunc(user_event_meta, meta_copy_func)
                                pyds.set_user_releasefunc(user_event_meta, meta_free_func)
                                pyds.nvds_add_user_meta_to_frame(frame_meta, user_event_meta)
                            else:
                                raise Exception("Error in attaching event meta to buffer")
                            is_first_object = False                
    logging.info("set_event_message_meta_probe: END")
    return Gst.PadProbeReturn.OK


def main():
    parser = argparse.ArgumentParser(
        description="Capture from RTSP camera, detect objects, write video data and metadata to a Pravega stream")
    parser.add_argument("--controller", default="192.168.1.123:9090")
    parser.add_argument("--log_level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--msgconv-config-file",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "msgconv_config.txt"))
    parser.add_argument("--pgie_config_file",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "pgie_config.txt"))
    parser.add_argument("-p", "--proto-lib", dest="proto_lib",
        help="Absolute path of adaptor library", metavar="PATH",
        default="/opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so")
    parser.add_argument("-s", "--schema-type", dest="schema_type", type=int, default=0,
        help="Type of message schema (0=Full, 1=minimal), default=0", metavar="<0|1>")
    parser.add_argument("--scope", default="examples")
    parser.add_argument("--source-uri", required=True)
    parser.add_argument("--data-stream", default="camera3")
    parser.add_argument("--metadata-stream", default="metadata3",
        help="Name of stream for metadata.", metavar="STREAM")
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    # Set GStreamer log level.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = ("WARNING,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO," +
            "h264parse:INFO,nvv4l2decoder:LOG,nvmsgconv:INFO,pravegasink:DEBUG")
    if not "PRAVEGA_PROTOCOL_ADAPTER_LOG" in os.environ:
        os.environ["PRAVEGA_PROTOCOL_ADAPTER_LOG"] = ("nvds_pravega_proto=trace,warn")

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    pipeline_description = (
        "rtspsrc name=source\n" +
        "   ! rtph264depay\n" +
        "   ! queue name=q_after_h264depay\n" +
        "   ! tee name=t\n" +
        "t. ! queue name=q_before_h264parse\n" +
        "   ! h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        "   ! mpegtsmux\n" +
        "   ! queue name=q_before_pravegasink\n" +
        "   ! pravegasink name=pravegasink\n" +
        "t. ! queue name=q_before_decode\n" +
        "   ! nvv4l2decoder name=decoder\n" +
        "   ! queue name=q_after_decode\n" +
        "   ! streammux.sink_0\n" +
        "nvstreammux name=streammux\n" +
        "   ! queue name=q_after_streammux\n" +
        "   ! nvinfer name=pgie\n" +
        "   ! queue name=q_after_infer\n" +
        "   ! nvstreamdemux name=streamdemux\n" +
        "streamdemux.src_0\n" +
        "   ! identity name=before_msgconv\n" +
        "   ! nvmsgconv name=msgconv\n" +
        "   ! nvmsgbroker name=msgbroker\n" +
        "")
    logging.info("Creating pipeline:\n" +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    source = pipeline.get_by_name("source")
    source.set_property("location", args.source_uri)
    source.set_property("buffer-mode", "none")
    source.set_property("drop-on-latency", True)
    source.set_property("latency", 2000)
    source.set_property("ntp-sync", True)
    source.set_property("ntp-time-source", "running-time")
    streammux = pipeline.get_by_name("streammux")
    if streammux:
        streammux.set_property("width", 1920)
        streammux.set_property("height", 1080)
        streammux.set_property("batch-size", 1)
        streammux.set_property("batched-push-timeout", 4000000)
        streammux.set_property("live-source", 1)
        streammux.set_property("attach-sys-ts", False)
    pgie = pipeline.get_by_name("pgie")
    if pgie:
        pgie.set_property("config-file-path", args.pgie_config_file)        
    msgconv = pipeline.get_by_name("msgconv")
    if msgconv:
        msgconv.set_property("config", args.msgconv_config_file)
        msgconv.set_property("payload-type", args.schema_type)
    msgbroker = pipeline.get_by_name("msgbroker")
    if msgbroker:
        msgbroker.set_property("proto-lib", args.proto_lib)
        msgbroker.set_property("conn-str", "pravega://%s" % args.controller)
        msgbroker.set_property("topic", "%s/%s" % (args.scope, args.metadata_stream))
        msgbroker.set_property("sync", False)
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("controller", args.controller)
        pravegasink.set_property("stream", "%s/%s" % (args.scope, args.data_stream))
        pravegasink.set_property("sync", False)
        pravegasink.set_property("timestamp-mode", "ntp")

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    # Add probe to add event message metadata to buffer.
    before_msgconv = pipeline.get_by_name("before_msgconv")
    if before_msgconv:
        before_msgconv_sinkpad = before_msgconv.get_static_pad("sink")
        if not before_msgconv_sinkpad:
            raise Exception("Unable to get sink pad of before_msgconv")
        before_msgconv_sinkpad.add_probe(Gst.PadProbeType.BUFFER, set_event_message_meta_probe, 0)

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
