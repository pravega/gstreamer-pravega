//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// A source that reads GStreamer buffers from a Pravega stream, as written by pravegasink.
// Based on:
//   - https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/master/generic/file/src/filesrc

use glib::prelude::*;
use glib::subclass::prelude::*;
use gst::ClockTime;
use gst::subclass::prelude::*;
use gst::{debug, error, info, log, trace, memdump};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;

use std::convert::{TryInto, TryFrom};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::u8;

use once_cell::sync::Lazy;

use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, StreamConfiguration, ScopedStream, Scaling, ScaleType};
use pravega_video::event_serde::EventReader;
use pravega_video::index::{IndexSearcher, get_index_stream_name};
use pravega_video::timestamp::PravegaTimestamp;
use pravega_video::utils;
use pravega_video::utils::{CurrentHead, SyncByteReader};
use crate::counting_reader::CountingReader;
use crate::seekable_take::SeekableTake;
use crate::utils::{clocktime_to_pravega, pravega_to_clocktime};

const PROPERTY_NAME_STREAM: &str = "stream";
const PROPERTY_NAME_CONTROLLER: &str = "controller";
const PROPERTY_NAME_BUFFER_SIZE: &str = "buffer-size";
const PROPERTY_NAME_START_MODE: &str = "start-mode";
const PROPERTY_NAME_END_MODE: &str = "end-mode";
const PROPERTY_NAME_START_TIMESTAMP: &str = "start-timestamp";
const PROPERTY_NAME_END_TIMESTAMP: &str = "end-timestamp";
const PROPERTY_NAME_START_UTC: &str = "start-utc";
const PROPERTY_NAME_END_UTC: &str = "end-utc";
const PROPERTY_NAME_ALLOW_CREATE_SCOPE: &str = "allow-create-scope";
const PROPERTY_NAME_KEYCLOAK_FILE: &str = "keycloak-file";

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstStartMode")]
pub enum StartMode {
    #[enum_type(
        name = "This element will not initiate a seek when starting. \
                It will begin reading from the first available buffer in the stream. \
                It will not use the index and it will not set the segment times. \
                This should generally not be used when playing with sync=true. \
                This option is only useful if you wish to read buffers that may exist prior to an index record.",
        nick = "no-seek"
    )]
    NoSeek = 0,
    #[enum_type(
        name = "Start at the earliest available random-access point.",
        nick = "earliest"
    )]
    Earliest = 1,
    #[enum_type(
        name = "Start at the most recent random-access point.",
        nick = "latest"
    )]
    Latest = 2,
    #[enum_type(
        name = "Start at the random-access point on or immediately before \
                the specified start-timestamp or start-utc. \
                The segment will start at the random-access point.",
        nick = "timestamp"
    )]
    Timestamp = 3,
    #[enum_type(
        name = "Start at the random-access point on or immediately before \
                the specified start-timestamp or start-utc. \
                The segment will start at the specified timestamp. \
                Buffers between the random-access point and the specified timestamp are expected to be dropped by decoders. \
                Use this for resuming a pipeline at a precise time.",
        nick = "timestamp-exact"
    )]
    TimestampExact = 4,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstEndMode")]
pub enum EndMode {
    #[enum_type(
        name = "Do not stop until the stream has been sealed.",
        nick = "unbounded"
    )]
    Unbounded = 0,
    #[enum_type(
        name = "Determine the last byte in the data stream when the pipeline starts. \
                Stop immediately after that byte has been emitted.",
        nick = "latest"
    )]
    Latest = 1,
    #[enum_type(
        name = "Search the index for the last record when the pipeline starts. \
                Stop immediately before the located position.",
        nick = "latest-indexed"
    )]
    LatestIndexed = 2,
    #[enum_type(
        name = "Search the index for the record on or immediately after \
                the specified end-timestamp or end-utc. \
                Stop immediately before the located position.",
        nick = "timestamp"
    )]
    Timestamp = 3,
}

const DEFAULT_BUFFER_SIZE: usize = 128*1024;
const DEFAULT_START_MODE: StartMode = StartMode::Earliest;
const DEFAULT_END_MODE: EndMode = EndMode::Unbounded;
const DEFAULT_START_TIMESTAMP: u64 = 0;
const DEFAULT_END_TIMESTAMP: u64 = u64::MAX;

#[derive(Debug)]
struct Settings {
    scope: Option<String>,
    stream: Option<String>,
    controller: Option<String>,
    buffer_size: usize,
    start_mode: StartMode,
    end_mode: EndMode,
    start_timestamp: u64,
    end_timestamp: u64,
    allow_create_scope: bool,
    keycloak_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scope: None,
            stream: None,
            controller: utils::default_pravega_controller_uri(),
            buffer_size: DEFAULT_BUFFER_SIZE,
            start_mode: DEFAULT_START_MODE,
            end_mode: DEFAULT_END_MODE,
            start_timestamp: DEFAULT_START_TIMESTAMP,
            end_timestamp: DEFAULT_END_TIMESTAMP,
            allow_create_scope: true,
            keycloak_file: utils::default_keycloak_file(),
        }
    }
}

enum State {
    Stopped,
    Started {
        reader: Arc<Mutex<CountingReader<BufReader<SeekableTake<SyncByteReader>>>>>,
        index_searcher: Arc<Mutex<IndexSearcher<SyncByteReader>>>,
        // save client facotry to keep the tokio runtime
        client_factory: ClientFactory,
    },
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

pub struct PravegaSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "pravegasrc",
        gst::DebugColorFlags::empty(),
        Some("Pravega Source"),
    )
});

impl PravegaSrc {
    fn set_stream(
        &self,
        stream: Option<String>,
    ) -> Result<(), glib::Error> {
        let obj = self.instance();
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
                info!(CAT, obj: obj, "Resetting `{}` to None", PROPERTY_NAME_STREAM);
                (None, None)
            }
        };
        settings.scope = scope;
        settings.stream = stream;
        Ok(())
    }

    fn set_controller(
        &self,
        controller: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        settings.controller = controller;
        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PravegaSrc {
    const NAME: &'static str = "PravegaSrc";
    type Type = super::PravegaSrc;
    type ParentType = gst_base::PushSrc;

    fn new() -> Self {
        pravega_video::tracing::init();
        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for PravegaSrc {
    fn constructed(&self) {
        self.parent_constructed();
        self.instance().set_format(gst::Format::Time);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpecString::new(
                PROPERTY_NAME_STREAM,
                "Stream",
                "scope/stream",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecString::new(
                PROPERTY_NAME_CONTROLLER,
                "Controller",
                format!("Pravega controller. \
                    If not specified, this will use the value of the environment variable {}. \
                    If that is empty, it will use the default of {}.",
                    utils::ENV_PRAVEGA_CONTROLLER_URI, utils::DEFAULT_PRAVEGA_CONTROLLER_URI).as_str(),
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecUInt::new(
                PROPERTY_NAME_BUFFER_SIZE,
                "Buffer size",
                "Size of buffer in number of bytes",
                0,
                std::u32::MAX,
                DEFAULT_BUFFER_SIZE.try_into().unwrap(),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecEnum::new(
                PROPERTY_NAME_START_MODE,
                "Start mode",
                "The position to start reading the stream at",
                StartMode::static_type(),
                DEFAULT_START_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecEnum::new(
                PROPERTY_NAME_END_MODE,
                "End mode",
                "The position to end reading the stream at",
                EndMode::static_type(),
                DEFAULT_END_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecUInt64::new(
                PROPERTY_NAME_START_TIMESTAMP,
                "Start timestamp",
                "If start-mode=timestamp, this is the timestamp at which to start, \
                in nanoseconds since 1970-01-01 00:00 TAI (International Atomic Time).",
                0,
                std::u64::MAX,
                DEFAULT_START_TIMESTAMP,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecUInt64::new(
                PROPERTY_NAME_END_TIMESTAMP,
                "End timestamp",
                "If end-mode=timestamp, this is the timestamp at which to stop, \
                in nanoseconds since 1970-01-01 00:00 TAI (International Atomic Time).",
                0,
                std::u64::MAX,
                DEFAULT_END_TIMESTAMP,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecString::new(
                PROPERTY_NAME_START_UTC,
                "Start UTC",
                "If start-mode=utc, this is the timestamp at which to start, \
                in RFC 3339 format. For example: 2021-12-28T23:41:45.691Z",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecString::new(
                PROPERTY_NAME_END_UTC,
                "End UTC",
                "If end-mode=utc, this is the timestamp at which to stop, \
                in RFC 3339 format. For example: 2021-12-28T23:41:45.691Z",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecBoolean::new(
                PROPERTY_NAME_ALLOW_CREATE_SCOPE,
                "Allow create scope",
                "If true, the Pravega scope will be created if needed.",
                true,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpecString::new(
                PROPERTY_NAME_KEYCLOAK_FILE,
                "Keycloak file",
                format!("The filename containing the Keycloak credentials JSON. \
                    If not specified, this will use the value of the environment variable {}. \
                    If that is empty, authentication will be disabled.",
                    utils::ENV_KEYCLOAK_SERVICE_ACCOUNT_FILE).as_str(),
                None,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    // TODO: On error, should set flag that will cause element to fail.
    fn set_property(
        &self,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        let obj = self.instance();
        match pspec.name() {
            PROPERTY_NAME_STREAM => {
                let res = match value.get::<String>() {
                    Ok(stream) => self.set_stream(Some(stream)),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_STREAM, err);
                }
            },
            PROPERTY_NAME_CONTROLLER => {
                let res = match value.get::<String>() {
                    Ok(controller) => {
                        let controller = if controller.is_empty() {
                            None
                        } else {
                            Some(controller)
                        };
                        self.set_controller(controller)
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_CONTROLLER, err);
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
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_BUFFER_SIZE, err);
                }
            },
            PROPERTY_NAME_START_MODE => {
                let res: Result<(), glib::Error> = match value.get::<StartMode>() {
                    Ok(start_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.start_mode = start_mode;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_MODE, err);
                }
            },
            PROPERTY_NAME_END_MODE => {
                let res: Result<(), glib::Error> = match value.get::<EndMode>() {
                    Ok(end_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.end_mode = end_mode;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_MODE, err);
                }
            },
            PROPERTY_NAME_START_TIMESTAMP => {
                let res: Result<(), glib::Error> = match value.get::<u64>() {
                    Ok(start_timestamp) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.start_timestamp = start_timestamp;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_TIMESTAMP, err);
                }
            },
            PROPERTY_NAME_END_TIMESTAMP => {
                let res: Result<(), glib::Error> = match value.get::<u64>() {
                    Ok(end_timestamp) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.end_timestamp = end_timestamp;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_TIMESTAMP, err);
                }
            },
            PROPERTY_NAME_START_UTC => {
                let res = match value.get::<String>() {
                    Ok(start_utc) => {
                        let mut settings = self.settings.lock().unwrap();
                        let timestamp = PravegaTimestamp::try_from(start_utc);
                        timestamp.map(|t| settings.start_timestamp = t.nanoseconds().unwrap())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_UTC, err);
                }
            },
            PROPERTY_NAME_END_UTC => {
                let res = match value.get::<String>() {
                    Ok(end_utc) => {
                        let mut settings = self.settings.lock().unwrap();
                        let timestamp = PravegaTimestamp::try_from(end_utc);
                        timestamp.map(|t| settings.end_timestamp = t.nanoseconds().unwrap())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_UTC, err);
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
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_ALLOW_CREATE_SCOPE, err);
                }
            },
            PROPERTY_NAME_KEYCLOAK_FILE => {
                let res: Result<(), glib::Error> = match value.get::<String>() {
                    Ok(keycloak_file) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.keycloak_file = if keycloak_file.is_empty() {
                            None
                        } else {
                            Some(keycloak_file)
                        };
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_KEYCLOAK_FILE, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

impl GstObjectImpl for PravegaSrc {}

impl ElementImpl for PravegaSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Pravega Source",
                "Source/Pravega",
                "Read from a Pravega stream",
                "Claudio Fahey <claudio.fahey@dell.com>",
                )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSrcImpl for PravegaSrc {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let obj = self.instance();
        debug!(CAT, obj: obj, "start: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Started { .. } = *state {
                unreachable!("PravegaSrc already started");
            }

            let settings = self.settings.lock().unwrap();
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
            info!(CAT, obj: obj, "start: scope={}, stream={}, index_stream={}", scope, stream, index_stream);
            info!(CAT, obj: obj, "start: start_mode={:?}, start_timestamp={:?}",
                settings.start_mode, PravegaTimestamp::from_nanoseconds(Some(settings.start_timestamp)));
            info!(CAT, obj: obj, "start: end_mode={:?}, end_timestamp={:?}",
                settings.end_mode, PravegaTimestamp::from_nanoseconds(Some(settings.end_timestamp)));

            let controller = settings.controller.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Controller is not defined"])
            })?;
            info!(CAT, obj: obj, "start: controller={}", controller);
            let keycloak_file = settings.keycloak_file.clone();
            info!(CAT, obj: obj, "start: keycloak_file={:?}", keycloak_file);
            let config = utils::create_client_config(controller, keycloak_file).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega client config: {}", error])
            })?;
            trace!(CAT, obj: obj, "start: config={:?}", config);
            info!(CAT, obj: obj, "start: controller_uri={}:{}", config.controller_uri.domain_name(), config.controller_uri.port());
            info!(CAT, obj: obj, "start: is_tls_enabled={}", config.is_tls_enabled);
            info!(CAT, obj: obj, "start: is_auth_enabled={}", config.is_auth_enabled);

            let client_factory = ClientFactory::new(config);
            let controller_client = client_factory.controller_client();
            let runtime = client_factory.runtime();

            // Create scope.
            info!(CAT, obj: obj, "start: allow_create_scope={}", settings.allow_create_scope);
            if settings.allow_create_scope {
                // This is expected to fail in some environments, even if the scope already exists.
                // We will log the error and continue.
                let _ = runtime.block_on(controller_client.create_scope(&scope)).map_err(|error| {
                    debug!(CAT, obj: obj, "Failed to create Pravega scope. This is normal if the scope already exists: {:?}", error);
                });
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
                tags: utils::get_video_tags(),
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
                tags: None,
            };
            runtime.block_on(controller_client.create_stream(&index_stream_config)).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega index stream: {:?}", error])
            })?;

            let scoped_stream = ScopedStream {
                scope: scope.clone(),
                stream: stream.clone(),
            };
            let reader = runtime.block_on(client_factory.create_byte_reader(scoped_stream));
            let mut reader = SyncByteReader::new(reader, client_factory.runtime_handle());
            info!(CAT, obj: obj, "start: Opened Pravega reader for data");

            let index_scoped_stream = ScopedStream {
                scope: scope.clone(),
                stream: index_stream.clone(),
            };
            let index_reader = runtime.block_on(client_factory.create_byte_reader(index_scoped_stream));
            info!(CAT, obj: obj, "start: Opened Pravega reader for index");

            let mut index_searcher = IndexSearcher::new(SyncByteReader::new(index_reader, client_factory.runtime_handle()));

            // TODO: Run below based on CAT threshold.
            // debug!(CAT, obj: obj, "index_records={:?}", index_searcher.get_index_records());

            // end_offset is the byte offset in the data stream.
            // The data stream reader will be configured to never read beyond this offset.
            let end_offset = match settings.end_mode {
                EndMode::Unbounded => u64::MAX,
                EndMode::Latest => {
                    // When ending at Latest, we will emit up through the very last byte currently in the data stream.
                    reader.seek(SeekFrom::End(0)).unwrap()
                },
                EndMode::LatestIndexed => {
                    // Determine Pravega stream offset for this timestamp by searching the index.
                    let index_record = index_searcher.get_last_record().unwrap();
                    info!(CAT, obj: obj, "start: end index_record={:?}", index_record);
                    index_record.offset
                },
                EndMode::Timestamp => {
                    let end_timestamp = PravegaTimestamp::from_nanoseconds(Some(settings.end_timestamp));
                    // Determine Pravega stream offset for this timestamp by searching the index.
                    let index_record = index_searcher.search_timestamp_after(end_timestamp).unwrap();
                    info!(CAT, obj: obj, "start: end index_record={:?}", index_record);
                    index_record.offset
                },
            };
            info!(CAT, obj: obj, "start: end_offset={}", end_offset);

            let limited_reader = SeekableTake::new(reader, end_offset).unwrap();
            let buf_reader = BufReader::with_capacity(settings.buffer_size, limited_reader);
            let counting_reader = CountingReader::new(buf_reader).unwrap();

            *state = State::Started {
                reader: Arc::new(Mutex::new(counting_reader)),
                index_searcher: Arc::new(Mutex::new(index_searcher)),
                client_factory,
            };
            info!(CAT, obj: obj, "start: Started");
            Ok(())
        })();
        debug!(CAT, obj: obj, "start: END: result={:?}", result);
        result
    }

    fn is_seekable(&self) -> bool {
        true
    }

    /// This method is called in the following scenarios:
    /// 1) initial_seek=true: It is first called right after start() returns.
    ///    The input segment times will all be 0.
    ///    If the start-mode parameter is no-seek:
    ///       a. This method will not use the index.
    ///       b. Reading will begin at the head of the stream.
    ///       c. All segment times will be 0.
    ///    Otherwise, this will use the index to locate the timestamp specified by the start-mode parameter.
    /// 2) initial_seek=false: It will be called when a GStreamer application performs a seek using GstElement.seek_simple().
    ///    The input segment time will be the number of nanoseconds since 1970-01-01 0:00:00 TAI.
    ///
    /// When using the index:
    /// 1) This method will find the last index record before or equal to the desired time.
    /// 2) The Pravega reader offset and the segment times will be set using
    ///    the values from the located index record.
    /// 3) The segment times will be set so that each buffer will have a PTS and position equal to
    ///    the number of nanoseconds since 1970-01-01 0:00:00 TAI.
    fn do_seek(&self, segment: &mut gst::Segment) -> bool {
        let obj = self.instance();
        info!(CAT, obj: obj, "do_seek: BEGIN: segment={:?}", segment);
        let result = (|| {
            // Get needed settings, then release lock.
            let (start_mode, initial_seek_start_timestamp) = {
                let settings = self.settings.lock().unwrap();
                let start_timestamp = match settings.start_mode {
                    StartMode::NoSeek => PravegaTimestamp::NONE,
                    StartMode::Earliest => {
                        // When starting at Earliest, the index will be used to find to the first random-access point.
                        PravegaTimestamp::MIN
                    },
                    StartMode::Latest => {
                        // When starting at Latest, the index will be used to find the last random-access point.
                        PravegaTimestamp::MAX
                    },
                    StartMode::Timestamp | StartMode::TimestampExact => {
                        // The index will be used to find a last random-access point before or on the specified timestamp.
                        PravegaTimestamp::from_nanoseconds(Some(settings.start_timestamp))
                    },
                };
                (settings.start_mode, start_timestamp)
            };

            let mut state = self.state.lock().unwrap();

            let (reader, index_searcher) = match *state {
                State::Started {
                    ref mut reader,
                    ref mut index_searcher,
                    ..
                } => (reader, index_searcher),
                State::Stopped => {
                    panic!("Not started yet");
                }
            };

            let reader = reader.clone();
            let index_searcher = index_searcher.clone();
            drop(state);
            let mut reader = reader.lock().unwrap();
            let mut index_searcher = index_searcher.lock().unwrap();

            let segment = segment.downcast_mut::<gst::format::Time>().unwrap();

            // In the input segment parameter, start, position, and time are all set to the desired timestamp.
            // If this is the initial seek, these will be all 0, and we will seek to the first record in the index.
            let initial_seek =
                segment.time().is_some() && segment.time().unwrap().nseconds() == 0 &&
                segment.start().is_some() && segment.start().unwrap().nseconds() == 0 &&
                segment.position().is_some() && segment.position().unwrap().nseconds() == 0;
            info!(CAT, obj: obj, "do_seek: initial_seek={}", initial_seek);
            let no_seek = initial_seek && start_mode == StartMode::NoSeek;
            let seek_using_index = !no_seek;
            if seek_using_index {
                let requested_seek_timestamp = if initial_seek {
                    initial_seek_start_timestamp
                } else {
                    clocktime_to_pravega(segment.time())
                };
                info!(CAT, obj: obj, "do_seek: seeking to timestamp {:?}", requested_seek_timestamp);
                // Determine the stream offset for this timestamp by searching the index.
                let index_record = index_searcher.search_timestamp(requested_seek_timestamp);
                info!(CAT, obj: obj, "do_seek: index_record={:?}", index_record);
                match index_record {
                    Ok(index_record) => {
                        let segment_start_timestamp = match start_mode {
                            StartMode::TimestampExact => {
                                // The segment will start at the requested timestamp.
                                requested_seek_timestamp
                            },
                            _ => {
                                // The segment will start at the indexed time.
                                index_record.timestamp
                            },                                
                        };
                        info!(CAT, obj: obj, "do_seek: segment will start at {:?}", segment_start_timestamp);
                        segment.set_start(pravega_to_clocktime(segment_start_timestamp));
                        segment.set_time(pravega_to_clocktime(segment_start_timestamp));
                        segment.set_position(Some(ClockTime::ZERO));
                        reader.seek(SeekFrom::Start(index_record.offset)).unwrap();
                        info!(CAT, obj: obj, "do_seek: seeked to indexed position; segment={:?}", segment);
                        true
                    },
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                        // This will happen if the index has no records.
                        // We cannot set the segment times appropriately.
                        error!(CAT, obj: obj, "do_seek: index is empty; segment={:?}", segment);
                        // TODO: Block until the first index record is read.
                        false
                    },
                    Err(_) => {
                        false
                    }
                }
            } else {
                // This is the initial seek and start-mode=no-seek.
                // The index will not be used.
                segment.set_start(Some(ClockTime::ZERO));
                segment.set_time(Some(ClockTime::ZERO));
                segment.set_position(Some(ClockTime::ZERO));
                let head_offset = reader.get_ref().get_ref().get_ref().current_head().unwrap();
                reader.seek(SeekFrom::Start(head_offset)).unwrap();
                info!(CAT, obj: obj, "do_seek: Starting at head of data stream because start-mode=no-seek; segment={:?}", segment);
                true
            }
        })();
        info!(CAT, obj: obj, "do_seek: END: result={:?}", result);
        result
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        let obj = self.instance();
        debug!(CAT, obj: obj, "query: BEGIN: query={:?}", query);
        let result = (|| {
            match query.view_mut() {
                // The Seeking query will return the current start and end timestamps
                // as nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI.
                gst::QueryViewMut::Seeking(ref mut q) => {
                    let fmt = q.format();
                    if fmt == gst::Format::Time {
                        // Get start and end timestamps from index.

                        // Get a temporary lock on state to get the index_searcher.
                        // This lock is released before index_searcher performs I/O.
                        let mut state = self.state.lock().unwrap();
                        let index_searcher = match *state {
                            State::Started {
                                ref mut index_searcher,
                                ..
                            } => index_searcher,
                            State::Stopped => {
                                return false;
                            }
                        };
                        let index_searcher = index_searcher.clone();
                        drop(state);
                        let mut index_searcher = index_searcher.lock().unwrap();

                        let start = match index_searcher.get_first_record() {
                            Ok(start) => start,
                            Err(err) => {
                                error!(CAT, obj: obj, "query: Unable to get first record from index: {}", err);
                                return false;
                            }
                        };
                        let end = match index_searcher.get_last_record() {
                            Ok(end) => end,
                            Err(err) => {
                                error!(CAT, obj: obj, "query: Unable to get last record from index: {}", err);
                                return false;
                            }
                        };
                        info!(CAT, obj: obj, "query: start={:?}, end={:?}", start, end);
                        q.set(true, pravega_to_clocktime(start.timestamp), pravega_to_clocktime(end.timestamp));
                        return true;
                    };
                    false
                },
                _ => {
                    BaseSrcImplExt::parent_query(self, query)
                },
            }
        })();
        debug!(CAT, obj: obj, "query: END: result={}, query={:?}", result, query);
        result
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let obj = self.instance();
        info!(CAT, obj: obj, "stop: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Stopped = *state {
                return Err(gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["PravegaSrc not started"]
                ));
            }
            *state = State::Stopped;
            Ok(())
        })();
        info!(CAT, obj: obj, "stop: END: result={:?}", result);
        result
    }
}

impl PushSrcImpl for PravegaSrc {
    fn create(&self, _buffer: Option<&mut gst::BufferRef>) -> Result<CreateSuccess, gst::FlowError> {
        let obj = self.instance();
        trace!(CAT, obj: obj, "create: BEGIN");
        let result = (|| {

            let mut state = self.state.lock().unwrap();

            let reader = match *state {
                State::Started {
                    ref mut reader,
                    ..
                } => reader,
                State::Stopped => {
                    gst::element_error!(obj, gst::CoreError::Failed, ["Not started yet"]);
                    panic!("Not started yet");
                }
            };

            let reader = reader.clone();
            drop(state);
            let mut reader = reader.lock().unwrap();
            let reader = &mut (*reader);

            let mut event_reader = EventReader::new();
            let offset = reader.stream_position().unwrap();
            let required_buffer_length = event_reader.read_required_buffer_length(reader).map_err(|err| {
                if err.kind() == ErrorKind::UnexpectedEof {
                    info!(CAT, obj: obj, "create: reached EOF when trying to read event length");
                    gst::FlowError::Eos
                } else {
                    gst::element_error!(obj, gst::CoreError::Failed, ["Failed to read event length from stream: {}", err]);
                    gst::FlowError::Error
                }
            })?;

            // TODO: Read directly into GstBuffer.
            let mut read_buffer: Vec<u8> = vec![0; required_buffer_length];
            let event = event_reader.read_event(reader, &mut read_buffer[..]).map_err(|err| {
                if err.kind() == ErrorKind::UnexpectedEof {
                    info!(CAT, obj: obj, "create: reached EOF when trying to read event payload");
                    gst::FlowError::Eos
                } else {
                    gst::element_error!(obj, gst::CoreError::Failed, ["Failed to read event payload from stream: {}", err]);
                    gst::FlowError::Error
                }
            })?;
            memdump!(CAT, obj: obj, "create: event={:?}", event);
            let offset_end = reader.stream_position().unwrap();

            let mut gst_buffer = gst::Buffer::with_size(event.payload.len()).unwrap();
            {
                let buffer_ref = gst_buffer.get_mut().unwrap();

                let segment = self
                    .instance()
                    .segment()
                    .downcast::<gst::format::Time>()
                    .unwrap();
                trace!(CAT, obj: obj, "create: segment={:?}", segment);
                let pts = pravega_to_clocktime(event.header.timestamp);
                log!(CAT, obj: obj, "create: timestamp={:?}, pts={}, payload_len={}",
                    event.header.timestamp, pts, event.payload.len());

                buffer_ref.set_pts(pts);
                buffer_ref.set_offset(offset);
                buffer_ref.set_offset_end(offset_end);
                if !event.header.random_access {
                    buffer_ref.set_flags(gst::BufferFlags::DELTA_UNIT);
                }
                if event.header.discontinuity {
                    buffer_ref.set_flags(gst::BufferFlags::DISCONT);
                }

                let mut buffer_map = buffer_ref.map_writable().unwrap();
                let slice = buffer_map.as_mut_slice();
                slice.copy_from_slice(event.payload);
            }

            Ok(CreateSuccess::NewBuffer(gst_buffer))
        })();
        trace!(CAT, obj: obj, "create: END: result={:?}", result);
        result
    }
}
