//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// A sink that writes GStreamer buffers to a Pravega stream.
// Based on:
//   - https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/master/generic/file/src/filesink

use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{gst_debug, gst_error, gst_fixme, gst_info, gst_log, gst_trace, gst_memdump};
use gst_base::subclass::prelude::*;

use std::cmp;
use std::convert::TryInto;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::sync::Mutex;

use once_cell::sync::Lazy;

use pravega_client::client_factory::ClientFactory;
use pravega_client::byte_stream::ByteStreamWriter;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment, StreamConfiguration, ScopedStream, Scaling, ScaleType};
use pravega_video::event_serde::{EventWithHeader, EventWriter};
use pravega_video::index::{IndexRecord, IndexRecordWriter, get_index_stream_name};
use pravega_video::timestamp::PravegaTimestamp;
use pravega_video::utils;

use crate::counting_writer::CountingWriter;
use crate::numeric::u64_to_i64_saturating_sub;
use crate::seekable_byte_stream_writer::SeekableByteStreamWriter;

const PROPERTY_NAME_STREAM: &str = "stream";
const PROPERTY_NAME_CONTROLLER: &str = "controller";
const PROPERTY_NAME_SEAL: &str = "seal";
const PROPERTY_NAME_BUFFER_SIZE: &str = "buffer-size";
const PROPERTY_NAME_TIMESTAMP_MODE: &str = "timestamp-mode";
const PROPERTY_NAME_INDEX_MIN_SEC: &str = "index-min-sec";
const PROPERTY_NAME_INDEX_MAX_SEC: &str = "index-max-sec";
const PROPERTY_NAME_ALLOW_CREATE_SCOPE: &str = "allow-create-scope";
const PROPERTY_NAME_KEYCLOAK_FILE: &str = "keycloak-file";

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::GEnum)]
#[repr(u32)]
#[genum(type_name = "GstTimestampMode")]
pub enum TimestampMode {
    #[genum(
        name = "Pipeline uses the realtime clock which provides nanoseconds \
                since the Unix epoch 1970-01-01 00:00:00 UTC, not including leap seconds.",
        nick = "realtime-clock"
    )]
    RealtimeClock = 0,
    #[genum(
        name = "Input buffer timestamps are nanoseconds \
                since the NTP epoch 1900-01-01 00:00:00 UTC, not including leap seconds. \
                Use this for buffers from rtspsrc (ntp-sync=true ntp-time-source=running-time).",
        nick = "ntp"
    )]
    Ntp = 1,
    #[genum(
        name = "Input buffer timestamps are nanoseconds \
                since 1970-01-01 00:00:00 TAI International Atomic Time, including leap seconds. \
                Use this for buffers from pravegasrc.",
        nick = "tai"
    )]
    Tai = 2,
}

const DEFAULT_CONTROLLER: &str = "127.0.0.1:9090";
const DEFAULT_BUFFER_SIZE: usize = 128*1024;
const DEFAULT_TIMESTAMP_MODE: TimestampMode = TimestampMode::RealtimeClock;
const DEFAULT_INDEX_MIN_SEC: f64 = 0.5;
const DEFAULT_INDEX_MAX_SEC: f64 = 10.0;

#[derive(Debug)]
struct Settings {
    scope: Option<String>,
    stream: Option<String>,
    controller: Option<String>,
    seal: bool,
    buffer_size: usize,
    timestamp_mode: TimestampMode,
    index_min_nanos: u64,
    index_max_nanos: u64,
    allow_create_scope: bool,
    keycloak_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scope: None,
            stream: None,
            controller: Some(DEFAULT_CONTROLLER.to_owned()),
            seal: false,
            buffer_size: DEFAULT_BUFFER_SIZE,
            timestamp_mode: DEFAULT_TIMESTAMP_MODE,
            index_min_nanos: (DEFAULT_INDEX_MIN_SEC * 1e9) as u64,
            index_max_nanos: (DEFAULT_INDEX_MAX_SEC * 1e9) as u64,
            allow_create_scope: true,
            keycloak_file: None,
        }
    }
}

enum State {
    Stopped,
    Started {
        client_factory: ClientFactory,
        writer: CountingWriter<BufWriter<SeekableByteStreamWriter>>,
        index_writer: ByteStreamWriter,
        last_index_time: PravegaTimestamp,
        // The timestamp that will be written to the index upon end-of-stream.
        final_timestamp: PravegaTimestamp,
        // The offset that will be written to the index upon end-of-stream.
        final_offset: Option<u64>,
        buffers_written: u64,
    },
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

pub struct PravegaSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "pravegasink",
        gst::DebugColorFlags::empty(),
        Some("Pravega Sink"),
    )
});

impl PravegaSink {
    fn set_stream(
        &self,
        element: &super::PravegaSink,
        stream: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        let (scope, stream) = match stream {
            Some(stream) => {
                let components: Vec<&str> = stream.split('/').collect();
                if components.len() != 2 {
                    return Err(glib::Error::new(
                        gst::URIError::BadUri,
                        format!("stream parameter '{}' is formatted incorrectly. It must be specified as scope/stream.", stream).as_str(),
                    ));
                }
                let scope = components[0].to_owned();
                let stream = components[1].to_owned();
                (Some(scope), Some(stream))
            }
            None => {
                gst_info!(CAT, obj: element, "Resetting `{}` to None", PROPERTY_NAME_STREAM);
                (None, None)
            }
        };
        settings.scope = scope;
        settings.stream = stream;
        Ok(())
    }

    fn set_controller(
        &self,
        _element: &super::PravegaSink,
        controller: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        settings.controller = controller;
        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PravegaSink {
    const NAME: &'static str = "PravegaSink";
    type Type = super::PravegaSink;
    type ParentType = gst_base::BaseSink;

    fn new() -> Self {
        pravega_video::tracing::init();
        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for PravegaSink {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.set_element_flags(gst::ElementFlags::PROVIDE_CLOCK | gst::ElementFlags::REQUIRE_CLOCK);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::new_string(
                PROPERTY_NAME_STREAM,
                "Stream",
                "scope/stream",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_CONTROLLER,
                "Controller",
                "Pravega controller",
                Some(DEFAULT_CONTROLLER),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_boolean(
                PROPERTY_NAME_SEAL,
                "Seal",
                "Seal Pravega stream when stopped",
                false,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_uint(
                PROPERTY_NAME_BUFFER_SIZE,
                "Buffer size",
                "Size of buffer in number of bytes",
                0,
                std::u32::MAX,
                DEFAULT_BUFFER_SIZE.try_into().unwrap(),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_enum(
                PROPERTY_NAME_TIMESTAMP_MODE,
                "Timestamp mode",
                "Timestamp mode used by the input",
                TimestampMode::static_type(),
                DEFAULT_TIMESTAMP_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_double(
                PROPERTY_NAME_INDEX_MIN_SEC,
                "Minimum index interval",
                "The minimum number of seconds between index records",
                0.0,
                std::f64::INFINITY,
                DEFAULT_INDEX_MIN_SEC.try_into().unwrap(),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_double(
                PROPERTY_NAME_INDEX_MAX_SEC,
                "Maximum index interval",
                "Force index record if one has not been created in this many seconds, even at delta frames.",
                0.0,
                std::f64::INFINITY,
                DEFAULT_INDEX_MAX_SEC.try_into().unwrap(),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_boolean(
                PROPERTY_NAME_ALLOW_CREATE_SCOPE,
                "Allow create scope",
                "If true, the Pravega scope will be created if needed.",
                true,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_KEYCLOAK_FILE,
                "Keycloak file",
                "The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.",
                None,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            PROPERTY_NAME_STREAM => {
                let res = match value.get::<String>() {
                    Ok(stream) => self.set_stream(&obj, Some(stream)),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_STREAM, err);
                }
            },
            PROPERTY_NAME_CONTROLLER => {
                let res = match value.get::<String>() {
                    Ok(controller) => self.set_controller(&obj, Some(controller)),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_CONTROLLER, err);
                }
            },
            PROPERTY_NAME_SEAL => {
                let res: Result<(), glib::Error> = match value.get::<bool>() {
                    Ok(seal) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.seal = seal;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_SEAL, err);
                }
            },
            PROPERTY_NAME_BUFFER_SIZE => {
                let res: Result<(), glib::Error> = match value.get::<u32>() {
                    Ok(buffer_size) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.buffer_size = buffer_size.try_into().unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_BUFFER_SIZE, err);
                }
            },
            PROPERTY_NAME_TIMESTAMP_MODE => {
                let res: Result<(), glib::Error> = match value.get::<TimestampMode>() {
                    Ok(timestamp_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.timestamp_mode = timestamp_mode;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_TIMESTAMP_MODE, err);
                }
            },
            PROPERTY_NAME_INDEX_MIN_SEC => {
                let res: Result<(), glib::Error> = match value.get::<f64>() {
                    Ok(index_min_sec) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.index_min_nanos = (index_min_sec * 1e9) as u64;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_INDEX_MIN_SEC, err);
                }
            },
            PROPERTY_NAME_INDEX_MAX_SEC => {
                let res: Result<(), glib::Error> = match value.get::<f64>() {
                    Ok(index_max_sec) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.index_max_nanos = (index_max_sec * 1e9) as u64;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_INDEX_MAX_SEC, err);
                }
            },
            PROPERTY_NAME_ALLOW_CREATE_SCOPE => {
                let res: Result<(), glib::Error> = match value.get::<bool>() {
                    Ok(allow_create_scope) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.allow_create_scope = allow_create_scope;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_ALLOW_CREATE_SCOPE, err);
                }
            },
            PROPERTY_NAME_KEYCLOAK_FILE => {
                let res: Result<(), glib::Error> = match value.get::<String>() {
                    Ok(keycloak_file) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.keycloak_file = Some(keycloak_file);
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_KEYCLOAK_FILE, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

impl ElementImpl for PravegaSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Pravega Sink",
                "Sink/Pravega",
                "Write to a Pravega stream",
                "Claudio Fahey <claudio.fahey@dell.com>",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }

    // We always want to use the realtime (Unix) clock, although it is ignored when timestamp-mode=ntp.
    fn provide_clock(&self, element: &Self::Type) -> Option<gst::Clock> {
        let clock = gst::SystemClock::obtain();
        let clock_type = gst::ClockType::Realtime;
        clock.set_property("clock-type", &clock_type).unwrap();
        let time = clock.time();
        gst_info!(CAT, obj: element, "provide_clock: Using clock_type={:?}, time={}, ({} ns)", clock_type, time, time.nanoseconds().unwrap());
        Some(clock)
    }
}

impl BaseSinkImpl for PravegaSink {
    fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        gst_debug!(CAT, obj: element, "start: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Started { .. } = *state {
                unreachable!("PravegaSink already started");
            }

            let settings = self.settings.lock().unwrap();
            gst_info!(CAT, obj: element, "start: index_min_nanos={}, index_max_nanos={}", settings.index_min_nanos, settings.index_max_nanos);
            if !(settings.index_min_nanos <= settings.index_max_nanos) {
                return Err(gst::error_msg!(gst::ResourceError::Settings,
                    ["{} must be <= {}", PROPERTY_NAME_INDEX_MIN_SEC, PROPERTY_NAME_INDEX_MAX_SEC]))
            };
            let scope_name: String = settings.scope.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Scope is not defined"])
            })?;
            let stream_name = settings.stream.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Stream is not defined"])
            })?;
            let index_stream_name = get_index_stream_name(&stream_name);
            let scope = Scope::from(scope_name);
            let stream = Stream::from(stream_name);
            let index_stream = Stream::from(index_stream_name);
            gst_info!(CAT, obj: element, "start: scope={}, stream={}, index_stream={}", scope, stream, index_stream);
            gst_info!(CAT, obj: element, "start: timestamp_mode={:?}", settings.timestamp_mode);

            let controller = settings.controller.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Controller is not defined"])
            })?;
            gst_info!(CAT, obj: element, "start: controller={}", controller);
            let keycloak_file = settings.keycloak_file.clone();
            gst_info!(CAT, obj: element, "start: keycloak_file={:?}", keycloak_file);
            let config = utils::create_client_config(controller, keycloak_file).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega client config: {}", error])
            })?;
            gst_debug!(CAT, obj: element, "start: config={:?}", config);
            gst_info!(CAT, obj: element, "start: controller_uri={}:{}", config.controller_uri.domain_name(), config.controller_uri.port());
            gst_info!(CAT, obj: element, "start: is_tls_enabled={}", config.is_tls_enabled);
            gst_info!(CAT, obj: element, "start: is_auth_enabled={}", config.is_auth_enabled);

            let client_factory = ClientFactory::new(config);
            let controller_client = client_factory.get_controller_client();
            let runtime = client_factory.get_runtime();

            // Create scope.
            gst_info!(CAT, obj: element, "start: allow_create_scope={}", settings.allow_create_scope);
            if settings.allow_create_scope {
                runtime.block_on(controller_client.create_scope(&scope)).map_err(|error| {
                    gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega scope: {:?}", error])
                })?;
            }

            // Create data stream.
            let stream_config = StreamConfiguration {
                scoped_stream: ScopedStream {
                    scope: scope.clone(),
                    stream: stream.clone(),
                },
                scaling: Scaling {
                    scale_type: ScaleType::FixedNumSegments,
                    min_num_segments: 1,
                    ..Default::default()
                },
                retention: Default::default(),
            };
            runtime.block_on(controller_client.create_stream(&stream_config)).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega data stream: {:?}", error])
            })?;

            // Create index stream.
            let index_stream_config = StreamConfiguration {
                scoped_stream: ScopedStream {
                    scope: scope.clone(),
                    stream: index_stream.clone(),
                },
                scaling: Scaling {
                    scale_type: ScaleType::FixedNumSegments,
                    min_num_segments: 1,
                    ..Default::default()
                },
                retention: Default::default(),
            };
            runtime.block_on(controller_client.create_stream(&index_stream_config)).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega index stream: {:?}", error])
            })?;

            let scoped_segment = ScopedSegment {
                scope: scope.clone(),
                stream: stream.clone(),
                segment: Segment::from(0),
            };
            let mut writer = client_factory.create_byte_stream_writer(scoped_segment);
            gst_info!(CAT, obj: element, "start: Opened Pravega writer for data");
            writer.seek_to_tail();

            let index_scoped_segment = ScopedSegment {
                scope: scope.clone(),
                stream: index_stream.clone(),
                segment: Segment::from(0),
            };
            let mut index_writer = client_factory.create_byte_stream_writer(index_scoped_segment);
            gst_info!(CAT, obj: element, "start: Opened Pravega writer for index");
            index_writer.seek_to_tail();

            let seekable_writer = SeekableByteStreamWriter::new(writer).unwrap();
            gst_info!(CAT, obj: element, "start: Buffer size is {}", settings.buffer_size);
            let buf_writer = BufWriter::with_capacity(settings.buffer_size, seekable_writer);
            let counting_writer = CountingWriter::new(buf_writer).unwrap();

            *state = State::Started {
                client_factory,
                writer: counting_writer,
                index_writer,
                last_index_time: PravegaTimestamp::NONE,
                final_timestamp: PravegaTimestamp::NONE,
                final_offset: None,
                buffers_written: 0,
            };
            gst_info!(CAT, obj: element, "start: Started");
            Ok(())
        })();
        gst_debug!(CAT, obj: element, "start: END; result={:?}", result);
        result
    }

    fn render(
        &self,
        element: &Self::Type,
        buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {

        gst_trace!(CAT, obj: element, "render: BEGIN: Rendering {:?}", buffer);
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            let (writer,
                index_writer,
                last_index_time,
                final_timestamp,
                final_offset,
                buffers_written) = match *state {
                State::Started {
                    ref mut writer,
                    ref mut index_writer,
                    ref mut last_index_time,
                    ref mut final_timestamp,
                    ref mut final_offset,
                    ref mut buffers_written,
                    ..
                } => (writer,
                    index_writer,
                    last_index_time,
                    final_timestamp,
                    final_offset,
                    buffers_written),
                State::Stopped => {
                    gst::element_error!(element, gst::CoreError::Failed, ["Not started yet"]);
                    return Err(gst::FlowError::Error);
                }
            };

            let pts = buffer.pts();
            let duration = buffer.duration();

            let map = buffer.map_readable().map_err(|_| {
                gst::element_error!(element, gst::CoreError::Failed, ["Failed to map buffer"]);
                gst::FlowError::Error
            })?;
            let payload = map.as_ref();

            let (timestamp_mode, index_min_nanos, index_max_nanos) = {
                let settings = self.settings.lock().unwrap();
                (settings.timestamp_mode, settings.index_min_nanos, settings.index_max_nanos)
            };

            let timestamp = match timestamp_mode {
                TimestampMode::RealtimeClock => {
                    // pts is time between beginning of play and beginning of this buffer.
                    // base_time is the value of the pipeline clock (time since Unix epoch) at the beginning of play.
                    PravegaTimestamp::from_unix_nanoseconds((element.base_time() + pts).nseconds())
                },
                TimestampMode::Ntp => {
                    // When receiving from rtspsrc (ntp-sync=true ntp-time-source=running-time),
                    // pts will be the number of nanoseconds since the NTP epoch 1900-01-01 00:00:00 UTC
                    // of when the video frame was observed by the camera.
                    // Note: base_time is the value of the pipeline clock at the beginning of play. It is ignored.
                    PravegaTimestamp::from_ntp_nanoseconds(pts.nseconds())
                },
                TimestampMode::Tai => {
                    PravegaTimestamp::from_nanoseconds(pts.nseconds())
                }
            };

            // Get the writer offset before writing. This offset will be used in the index.
            let writer_offset = writer.seek(SeekFrom::Current(0)).unwrap();

            gst_log!(CAT, obj: element, "render: timestamp={:?}, pts={}, base_time={}, duration={}, size={}, writer_offset={}",
                timestamp, pts, element.base_time(), buffer.duration(), buffer.size(), writer_offset);

            // We only want to include key frames (non-delta units) in the index.
            // However, if no key frame has been received in a while, force an index record.
            // This is required for nvv4l2h264enc because it identifies all buffers as DELTA_UNIT.
            let buffer_flags = buffer.flags();
            let is_delta_unit = buffer_flags.contains(gst::BufferFlags::DELTA_UNIT);
            let random_access = !is_delta_unit;
            let include_in_index = match timestamp.nanoseconds() {
                Some(timestamp) => {
                    match last_index_time.nanoseconds() {
                        Some(last_index_time) => {
                            let interval_sec = u64_to_i64_saturating_sub(timestamp, last_index_time) as f64 * 1e-9;
                            if is_delta_unit {
                                if timestamp > last_index_time + index_max_nanos {
                                    gst_fixme!(CAT, obj: element,
                                        "render: Forcing index record at delta unit because no key frame has been received for {} sec", interval_sec);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                // We are at a key frame.
                                if timestamp < last_index_time + index_min_nanos {
                                    gst_debug!(CAT, obj: element,
                                        "render: Skipping creation of index record because an index record was created {} sec ago", interval_sec);
                                    false
                                } else {
                                    gst_debug!(CAT, obj: element,
                                        "render: Creating index record at key frame; last index record was created {} sec ago", interval_sec);
                                    true
                                }
                            }
                        },
                        None => {
                            // An index record has not been written by this element yet.
                            true
                        },
                    }
                },
                None => {
                    // Buffer has an invalid timestamp.
                    false
                },
            };

            // Per the index constraints defined in index.rs, if we are writing an index record now,
            // we must flush any data writes prior to this buffer, so that reads do not block waiting on this writer.
            let flush = include_in_index;
            if flush {
                writer.flush().map_err(|error| {
                    gst::element_error!(element, gst::CoreError::Failed, ["Failed to flush Pravega data stream: {}", error]);
                    gst::FlowError::Error
                })?;
            }

            // Record a discontinuity if any of the following are true:
            //   1) upstream has indicated a discontinuity (or resync) in the buffer
            //   3) this will be the first buffer written to the data stream from this instance
            //   2) this will be the first index record written from this instance
            let discontinuity =
                   buffer_flags.contains(gst::BufferFlags::DISCONT)
                || buffer_flags.contains(gst::BufferFlags::RESYNC)
                || *buffers_written == 0
                || (include_in_index && last_index_time.nanoseconds().is_none());
            if discontinuity {
                gst_debug!(CAT, obj: element, "render: Recording discontinuity");
            }

            // Write index record.
            // We write the index record before the buffer so that any readers blocked on reading the
            // index will unblock as soon as possible.
            if include_in_index {
                let index_record = IndexRecord::new(timestamp, writer_offset,
                    random_access, discontinuity);
                let mut index_record_writer = IndexRecordWriter::new();
                index_record_writer.write(&index_record, index_writer).map_err(|err| {
                    gst::element_error!(
                        element,
                        gst::ResourceError::Write,
                        ["Failed to write index: {}", err]
                    );
                    gst::FlowError::Error
                })?;
                gst_debug!(CAT, obj: element, "render: Wrote index record {:?}", index_record);
                *last_index_time = timestamp;
            }

            // Write buffer to Pravega byte stream.
            let event = EventWithHeader::new(payload, timestamp,
                include_in_index, random_access, discontinuity);
            gst_memdump!(CAT, obj: element, "render: writing event={:?}", event);
            let mut event_writer = EventWriter::new();
            event_writer.write(&event, writer).map_err(|err| {
                gst::element_error!(
                    element,
                    gst::ResourceError::Write,
                    ["Failed to write buffer: {}", err]
                );
                gst::FlowError::Error
            })?;
            *buffers_written += 1;

            // Get the writer offset after writing.
            let writer_offset_end = writer.seek(SeekFrom::Current(0)).unwrap();
            gst_trace!(CAT, obj: element, "render: wrote {} bytes from offset {} to {}",
                writer_offset_end - writer_offset, writer_offset, writer_offset_end);

            // Flush after writing if the buffer contains the SYNC_AFTER flag. This is normally not used.
            let sync_after = buffer_flags.contains(gst::BufferFlags::SYNC_AFTER);
            if sync_after {
                writer.flush().map_err(|error| {
                    gst::element_error!(element, gst::CoreError::Failed, ["Failed to flush Pravega data stream: {}", error]);
                    gst::FlowError::Error
                })?;
                index_writer.flush().map_err(|error| {
                    gst::element_error!(element, gst::CoreError::Failed, ["Failed to flush Pravega index stream: {}", error]);
                    gst::FlowError::Error
                })?;
                gst_debug!(CAT, obj: element, "render: Streams flushed because SYNC_AFTER flag was set");
            }

            // Maintain values that may be written to the index on end-of-stream.
            // Per the index constraints defined in index.rs, the timestamp in the index record must
            // be strictly greater than the timestamp in the data stream.
            if timestamp.is_some() {
                // If duration of the buffer is reported as 0, we record it as if it had a 1 nanosecond duration.
                let duration = cmp::max(1, duration.nanoseconds().unwrap_or_default());
                *final_timestamp = PravegaTimestamp::from_nanoseconds(
                    timestamp.nanoseconds().map(|t| t + duration));
            }
            *final_offset = Some(writer_offset_end);

            Ok(gst::FlowSuccess::Ok)
        })();
        gst_trace!(CAT, obj: element, "render: END: result={:?}", result);
        result
    }

    fn stop(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        gst_info!(CAT, obj: element, "stop: BEGIN");
        let result = (|| {
            let seal = {
                let settings = self.settings.lock().unwrap();
                settings.seal
            };

            let mut state = self.state.lock().unwrap();
            let (writer,
                index_writer,
                client_factory,
                final_timestamp,
                final_offset) = match *state {
                State::Started {
                    ref mut writer,
                    ref mut index_writer,
                    ref mut client_factory,
                    ref mut final_timestamp,
                    ref mut final_offset,
                    ..
                } => (writer,
                    index_writer,
                    client_factory,
                    final_timestamp,
                    final_offset),
                State::Stopped => {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Settings,
                        ["PravegaSink not started"]
                    ));
                }
            };

            writer.flush().map_err(|error| {
                gst::error_msg!(gst::ResourceError::Write, ["Failed to flush Pravega data stream: {}", error])
            })?;

            // Write final index record.
            // The timestamp will be the the buffer timestamp + duration of the final buffer.
            // The offset will be current write position.
            if let Some(final_offset) = *final_offset {
                if final_timestamp.is_some() {
                    let index_record = IndexRecord::new(*final_timestamp, final_offset,
                        false, false);
                    let mut index_record_writer = IndexRecordWriter::new();
                    index_record_writer.write(&index_record, index_writer).map_err(|error| {
                        gst::error_msg!(gst::ResourceError::Write, ["Failed to write Pravega index stream: {}", error])
                    })?;
                    gst_info!(CAT, obj: element, "stop: Wrote final index record {:?}", index_record);
                }
            }

            index_writer.flush().map_err(|error| {
                gst::error_msg!(gst::ResourceError::Write, ["Failed to flush Pravega index stream: {}", error])
            })?;

            if seal {
                gst_info!(CAT, obj: element, "stop: Sealing streams");
                let writer = writer.get_mut().get_mut().get_mut();
                client_factory.get_runtime().block_on(writer.seal()).map_err(|error| {
                    gst::error_msg!(gst::ResourceError::Write, ["Failed to seal Pravega data stream: {}", error])
                })?;
                client_factory.get_runtime().block_on(index_writer.seal()).map_err(|error| {
                    gst::error_msg!(gst::ResourceError::Write, ["Failed to seal Pravega index stream: {}", error])
                })?;
                gst_info!(CAT, obj: element, "stop: Streams sealed");
            }

            *state = State::Stopped;
            Ok(())
        })();
        gst_info!(CAT, obj: element, "stop: END: result={:?}", result);
        result
    }
}
