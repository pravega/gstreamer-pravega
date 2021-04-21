// A source that reads GStreamer buffers along with timestamps, as written by pravegasink.

use glib::subclass::prelude::*;
use gst::ClockTime;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{gst_error, gst_info, gst_log, gst_trace};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use std::convert::{TryInto, TryFrom};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::u8;
use std::env;
use std::collections::HashMap;

use once_cell::sync::Lazy;

use pravega_client::client_factory::ClientFactory;
use pravega_client::byte_stream::ByteStreamReader;
use pravega_client_config::ClientConfigBuilder;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment, StreamConfiguration, ScopedStream, Scaling, ScaleType};
use pravega_video::event_serde::EventReader;
use pravega_video::index::{IndexSearcher, get_index_stream_name};
use pravega_video::timestamp::PravegaTimestamp;
use pravega_video::utils;
use crate::seekable_take::SeekableTake;

const PROPERTY_NAME_STREAM: &str = "stream";
const PROPERTY_NAME_CONTROLLER: &str = "controller";
const PROPERTY_NAME_BUFFER_SIZE: &str = "buffer-size";
const PROPERTY_NAME_START_PTS_AT_ZERO: &str = "start-pts-at-zero";
const PROPERTY_NAME_START_MODE: &str = "start-mode";
const PROPERTY_NAME_END_MODE: &str = "end-mode";
const PROPERTY_NAME_START_TIMESTAMP: &str = "start-timestamp";
const PROPERTY_NAME_END_TIMESTAMP: &str = "end-timestamp";
const PROPERTY_NAME_START_UTC: &str = "start-utc";
const PROPERTY_NAME_END_UTC: &str = "end-utc";
const PROPERTY_NAME_ALLOW_CREATE_SCOPE: &str = "allow-create-scope";

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::GEnum)]
#[repr(u32)]
#[genum(type_name = "GstStartMode")]
pub enum StartMode {
    #[genum(
        name = "This element will not initiate a seek when starting. \
                Usually a pipeline will start with a seek to position 0, \
                in which case this would be equivalent to earliest.",
        nick = "no-seek"
    )]
    NoSeek = 0,
    #[genum(
        name = "Start at the earliest available random-access point.",
        nick = "earliest"
    )]
    Earliest = 1,
    #[genum(
        name = "Start at the most recent random-access point.",
        nick = "latest"
    )]
    Latest = 2,
    #[genum(
        name = "Start at the random-access point on or immediately before \
                the specified start-timestamp or start-utc.",
        nick = "timestamp"
    )]
    Timestamp = 3,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::GEnum)]
#[repr(u32)]
#[genum(type_name = "GstEndMode")]
pub enum EndMode {
    #[genum(
        name = "Do not stop until the stream has been sealed.",
        nick = "unbounded"
    )]
    Unbounded = 0,
    #[genum(
        name = "Determine the last byte in the data stream when the pipeline starts. \
                Stop immediately after that byte has been emitted.",
        nick = "latest"
    )]
    Latest = 1,
    #[genum(
        name = "Search the index for the last record when the pipeline starts. \
                Stop immediately before the located position.",
        nick = "latest-indexed"
    )]
    LatestIndexed = 2,
    #[genum(
        name = "Search the index for the record on or immediately after \
                the specified end-timestamp or end-utc. \
                Stop immediately before the located position.",
        nick = "timestamp"
    )]
    Timestamp = 3,
}

const DEFAULT_CONTROLLER: &str = "127.0.0.1:9090";
const DEFAULT_BUFFER_SIZE: usize = 128*1024;
const DEFAULT_START_PTS_AT_ZERO: bool = false;
const DEFAULT_START_MODE: StartMode = StartMode::NoSeek;
const DEFAULT_END_MODE: EndMode = EndMode::Unbounded;
const DEFAULT_START_TIMESTAMP: u64 = 0;
const DEFAULT_END_TIMESTAMP: u64 = u64::MAX;
const AUTH_KEYCLOAK_PATH: &str = "pravega_client_auth_keycloak";

#[derive(Debug)]
struct Settings {
    scope: Option<String>,
    stream: Option<String>,
    controller: Option<String>,
    buffer_size: usize,
    start_pts_at_zero: bool,
    start_mode: StartMode,
    end_mode: EndMode,
    start_timestamp: u64,
    end_timestamp: u64,
    allow_create_scope: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scope: None,
            stream: None,
            controller: Some(DEFAULT_CONTROLLER.to_owned()),
            buffer_size: DEFAULT_BUFFER_SIZE,
            start_pts_at_zero: DEFAULT_START_PTS_AT_ZERO,
            start_mode: DEFAULT_START_MODE,
            end_mode: DEFAULT_END_MODE,
            start_timestamp: DEFAULT_START_TIMESTAMP,
            end_timestamp: DEFAULT_END_TIMESTAMP,
            allow_create_scope: true,
        }
    }
}

enum State {
    Stopped,
    Started {
        reader: Arc<Mutex<BufReader<SeekableTake<ByteStreamReader>>>>,
        index_searcher: Arc<Mutex<IndexSearcher<ByteStreamReader>>>,
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
        element: &super::PravegaSrc,
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
        _element: &super::PravegaSrc,
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
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.set_format(gst::Format::Time);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::string(
                PROPERTY_NAME_STREAM,
                "Stream",
                "scope/stream",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::string(
                PROPERTY_NAME_CONTROLLER,
                "Controller",
                "Pravega controller",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::uint(
                PROPERTY_NAME_BUFFER_SIZE,
                "Buffer size",
                "Size of buffer in number of bytes",
                0,
                std::u32::MAX,
                DEFAULT_BUFFER_SIZE.try_into().unwrap(),
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::boolean(
                PROPERTY_NAME_START_PTS_AT_ZERO,
                "Start PTS at 0",
                "If true, the first buffer will have a PTS of 0. \
                If false, buffers will have a PTS equal to the raw timestamp stored in the Pravega stream \
                (nanoseconds since 1970-01-01 00:00 TAI International Atomic Time). \
                Use true when using sinks with sync=true such as an autoaudiosink. \
                Use false when using sinks with sync=false such as pravegasink.",
                DEFAULT_START_PTS_AT_ZERO,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::enum_(
                PROPERTY_NAME_START_MODE,
                "Start mode",
                "The position to start reading the stream at",
                StartMode::static_type(),
                DEFAULT_START_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::enum_(
                PROPERTY_NAME_END_MODE,
                "End mode",
                "The position to end reading the stream at",
                EndMode::static_type(),
                DEFAULT_END_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::uint64(
                PROPERTY_NAME_START_TIMESTAMP,
                "Start timestamp",
                "If start-mode=timestamp, this is the timestamp at which to start, \
                in nanoseconds since 1970-01-01 00:00 TAI (International Atomic Time).",
                0,
                std::u64::MAX,
                DEFAULT_START_TIMESTAMP,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::uint64(
                PROPERTY_NAME_END_TIMESTAMP,
                "End timestamp",
                "If end-mode=timestamp, this is the timestamp at which to stop, \
                in nanoseconds since 1970-01-01 00:00 TAI (International Atomic Time).",
                0,
                std::u64::MAX,
                DEFAULT_END_TIMESTAMP,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::string(
                PROPERTY_NAME_START_UTC,
                "Start UTC",
                "If start-mode=utc, this is the timestamp at which to start, \
                in RFC 3339 format. For example: 2021-12-28T23:41:45.691Z",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::string(
                PROPERTY_NAME_END_UTC,
                "End UTC",
                "If end-mode=utc, this is the timestamp at which to stop, \
                in RFC 3339 format. For example: 2021-12-28T23:41:45.691Z",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::boolean(
                PROPERTY_NAME_ALLOW_CREATE_SCOPE,
                "Allow create scope",
                "Controller whether to create scope at startup",
                true,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    // TODO: On error, should set flag that will cause element to fail.
    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.get_name() {
            PROPERTY_NAME_STREAM => {
                let res = match value.get::<String>() {
                    Ok(Some(stream)) => self.set_stream(&obj, Some(stream)),
                    Ok(None) => self.set_stream(&obj, None),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_STREAM, err);
                }
            },
            PROPERTY_NAME_CONTROLLER => {
                let res = match value.get::<String>() {
                    Ok(controller) => self.set_controller(&obj, controller),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_CONTROLLER, err);
                }
            },
            PROPERTY_NAME_BUFFER_SIZE => {
                let res: Result<(), glib::Error> = match value.get::<u32>() {
                    Ok(buffer_size) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.buffer_size = buffer_size.unwrap_or_default().try_into().unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_BUFFER_SIZE, err);
                }
            },
            PROPERTY_NAME_START_PTS_AT_ZERO => {
                let res: Result<(), glib::Error> = match value.get::<bool>() {
                    Ok(start_pts_at_zero) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.start_pts_at_zero = start_pts_at_zero.unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property {}: {}", PROPERTY_NAME_START_PTS_AT_ZERO, err);
                }
            },
            PROPERTY_NAME_START_MODE => {
                let res: Result<(), glib::Error> = match value.get::<StartMode>() {
                    Ok(start_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.start_mode = start_mode.unwrap();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_MODE, err);
                }
            },
            PROPERTY_NAME_END_MODE => {
                let res: Result<(), glib::Error> = match value.get::<EndMode>() {
                    Ok(end_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.end_mode = end_mode.unwrap();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_MODE, err);
                }
            },
            PROPERTY_NAME_START_TIMESTAMP => {
                let res: Result<(), glib::Error> = match value.get::<u64>() {
                    Ok(start_timestamp) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.start_timestamp = start_timestamp.unwrap_or_default().try_into().unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_TIMESTAMP, err);
                }
            },
            PROPERTY_NAME_END_TIMESTAMP => {
                let res: Result<(), glib::Error> = match value.get::<u64>() {
                    Ok(end_timestamp) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.end_timestamp = end_timestamp.unwrap_or_default().try_into().unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_TIMESTAMP, err);
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
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_START_UTC, err);
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
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_END_UTC, err);
                }
            },
            PROPERTY_NAME_ALLOW_CREATE_SCOPE => {
                let res: Result<(), glib::Error> = match value.get::<bool>() {
                    Ok(allow_create_scope) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.allow_create_scope = allow_create_scope.unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_ALLOW_CREATE_SCOPE, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

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
    fn start(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
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
        gst_info!(CAT, obj: element, "scope={}, stream={}, index_stream={}", scope, stream, index_stream);
        gst_info!(CAT, obj: element, "start_mode={:?}, end_mode={:?}", settings.start_mode, settings.end_mode);

        let controller = settings.controller.clone().ok_or_else(|| {
            gst::error_msg!(gst::ResourceError::Settings, ["Controller is not defined"])
        })?;
        gst_info!(CAT, obj: element, "controller={}", controller);
        let controller_uri = utils::parse_controller_uri(controller).unwrap();
        gst_info!(CAT, obj: element, "controller_uri={}", controller_uri);

        gst_info!(CAT, obj: element, "allow_create_scope={}", settings.allow_create_scope);

        let filter_env_val = env::vars()
            .filter(|(k, _v)| k.starts_with(AUTH_KEYCLOAK_PATH))
            .collect::<HashMap<String, String>>();
        let is_auth_enabled = if filter_env_val.contains_key(AUTH_KEYCLOAK_PATH) { true } else { false };
        gst_info!(CAT, obj: element, "is_auth_enabled={}", is_auth_enabled);

        let config = ClientConfigBuilder::default()
            .controller_uri(controller_uri)
            .is_auth_enabled(is_auth_enabled)
            .build()
            .expect("creating config");

        let client_factory = ClientFactory::new(config);
        let controller_client = client_factory.get_controller_client();
        let runtime = client_factory.get_runtime();

        // Create scope.
        if settings.allow_create_scope {
            runtime.block_on(controller_client.create_scope(&scope)).unwrap();
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
        runtime.block_on(controller_client.create_stream(&stream_config)).unwrap();

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
        runtime.block_on(controller_client.create_stream(&index_stream_config)).unwrap();

        let scoped_segment = ScopedSegment {
            scope: scope.clone(),
            stream: stream.clone(),
            segment: Segment::from(0),
        };
        let mut reader = client_factory.create_byte_stream_reader(scoped_segment);
        gst_info!(CAT, obj: element, "Opened Pravega reader");

        let index_scoped_segment = ScopedSegment {
            scope: scope.clone(),
            stream: index_stream.clone(),
            segment: Segment::from(0),
        };
        let index_reader = client_factory.create_byte_stream_reader(index_scoped_segment);
        gst_info!(CAT, obj: element, "Opened Pravega reader for index");

        let mut index_searcher = IndexSearcher::new(index_reader);

        let start_timestamp = match settings.start_mode {
            StartMode::NoSeek => PravegaTimestamp::NONE,
            StartMode::Earliest => {
                // When start at Earliest, the index will be used to find to the first random-access point.
                PravegaTimestamp::MIN
            },
            StartMode::Latest => {
                // When starting at Latest, the index will be used to find the last random-access point.
                PravegaTimestamp::MAX
            },
            StartMode::Timestamp => {
                // The index will be used to find a last random-access point before or on the specified timestamp.
                PravegaTimestamp::from_nanoseconds(Some(settings.start_timestamp))
            },
        };
        gst_info!(CAT, obj: element, "start_timestamp={}", start_timestamp);

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
                gst_info!(CAT, obj: element, "end index_record={:?}", index_record);
                index_record.offset
            },
            EndMode::Timestamp => {
                let end_timestamp = PravegaTimestamp::from_nanoseconds(Some(settings.end_timestamp));
                // Determine Pravega stream offset for this timestamp by searching the index.
                let index_record = index_searcher.search_timestamp_after(end_timestamp).unwrap();
                gst_info!(CAT, obj: element, "end index_record={:?}", index_record);
                index_record.offset
            },
        };
        gst_info!(CAT, obj: element, "end_offset={}", end_offset);

        let limited_reader = SeekableTake::new(reader, end_offset).unwrap();
        let buf_reader = BufReader::with_capacity(settings.buffer_size, limited_reader);

        *state = State::Started {
            reader: Arc::new(Mutex::new(buf_reader)),
            index_searcher: Arc::new(Mutex::new(index_searcher)),
        };
        // We must unlock the state so that seek does not deadlock.
        drop(state);
        gst_info!(CAT, obj: element, "Started");

        if let Some(seek_pos) = start_timestamp.nanoseconds() {
            element.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                seek_pos * gst::NSECOND,
            ).unwrap();
        }

        Ok(())
    }

    fn is_seekable(&self, _src: &Self::Type) -> bool {
        true
    }

    // This method is called in the following scenarios:
    // 1) It is first called right after we initialize the Pravega reader.
    //    The input segment time will be 0.
    //    This method will read the first index record.
    // 2) It will be called when an GStreamer application performs a seek using GstElement.seek_simple().
    //    The input segment time will be the number of nanoseconds since 1970-01-01 0:00:00 TAI.
    //    This method will find the last index record before or equal to the desired time.
    // In either case, the Pravega reader offset and the segment time will be set using
    // the values from the located index record.
    fn do_seek(&self, src: &Self::Type, segment: &mut gst::Segment) -> bool {
        gst_info!(CAT, obj: src, "do_seek: BEGIN: segment={:?}", segment);

        let start_pts_at_zero = {
            let settings = self.settings.lock().unwrap();
            settings.start_pts_at_zero
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
        segment.set_start(0);
        segment.set_position(0);
        let timestamp = segment.get_time().nseconds().unwrap();
        let timestamp = PravegaTimestamp::from_nanoseconds(Some(timestamp));
        // Determine Pravega stream offset for this timestamp by searching the index.
        let index_record = index_searcher.search_timestamp(timestamp);
        gst_info!(CAT, obj: src, "do_seek: index_record={:?}", index_record);
        match index_record {
            Ok(index_record) => {
                if start_pts_at_zero {
                    segment.set_time(ClockTime(index_record.timestamp.nanoseconds()));
                }
                reader.seek(SeekFrom::Start(index_record.offset)).unwrap();
                gst_info!(CAT, obj: src, "do_seek: END: segment={:?}", segment);
                true
            },
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                // This will happen if the index has no records.
                let head_offset = reader.get_ref().get_ref().current_head().unwrap();
                reader.seek(SeekFrom::Start(head_offset)).unwrap();
                gst_info!(CAT, obj: src, "do_seek: END: seeked to head because index is empty; segment={:?}", segment);
                true
            },
            Err(_) => {
                false
            }
        }
    }

    fn query(&self, src: &Self::Type, query: &mut gst::QueryRef) -> bool {
        gst_info!(CAT, obj: src, "query: query={:?}", query);
        match query.view_mut() {
            gst::QueryView::Seeking(ref mut q) => {
                let fmt = q.get_format();
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
                            gst_error!(CAT, obj: src, "query: Unable to get first record from index: {}", err);
                            return false;
                        }
                    };
                    let end = match index_searcher.get_last_record() {
                        Ok(end) => end,
                        Err(err) => {
                            gst_error!(CAT, obj: src, "query: Unable to get last record from index: {}", err);
                            return false;
                        }
                    };
                    gst_info!(CAT, obj: src, "query: start={:?}, end={:?}", start, end);
                    q.set(true, ClockTime(start.timestamp.nanoseconds()), ClockTime(end.timestamp.nanoseconds()));
                    return true;
                };
                false
            },
            _ => {
                BaseSrcImplExt::parent_query(self, src, query)
            },
        }
    }

    fn stop(&self, element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        gst_info!(CAT, obj: element, "Stopping");
        let mut state = self.state.lock().unwrap();
        if let State::Stopped = *state {
            return Err(gst::error_msg!(
                gst::ResourceError::Settings,
                ["PravegaSrc not started"]
            ));
        }
        *state = State::Stopped;
        gst_info!(CAT, obj: element, "Stopped");
        Ok(())
    }
}

impl PushSrcImpl for PravegaSrc {
    fn create(&self, element: &Self::Type) -> Result<gst::Buffer, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        let reader = match *state {
            State::Started {
                ref mut reader,
                ..
            } => reader,
            State::Stopped => {
                gst::element_error!(element, gst::CoreError::Failed, ["Not started yet"]);
                panic!("Not started yet");
            }
        };

        let reader = reader.clone();
        drop(state);
        let mut reader = reader.lock().unwrap();
        let reader = &mut (*reader);

        let mut event_reader = EventReader::new();
        let required_buffer_length = event_reader.read_required_buffer_length(reader).map_err(|err| {
            if err.kind() == ErrorKind::UnexpectedEof {
                gst_info!(CAT, obj: element, "create: reached EOF when trying to read event length");
                gst::FlowError::Eos
            } else {
                gst::element_error!(element, gst::CoreError::Failed, ["Failed to read event length from stream: {}", err]);
                gst::FlowError::Error
            }
        })?;

        // TODO: Read directly into GstBuffer.
        let mut read_buffer: Vec<u8> = vec![0; required_buffer_length];
        let event = event_reader.read_event(reader, &mut read_buffer[..]).map_err(|err| {
            if err.kind() == ErrorKind::UnexpectedEof {
                gst_info!(CAT, obj: element, "create: reached EOF when trying to read event payload");
                gst::FlowError::Eos
            } else {
                gst::element_error!(element, gst::CoreError::Failed, ["Failed to read event payload from stream: {}", err]);
                gst::FlowError::Error
            }
        })?;
        gst_trace!(CAT, obj: element, "create: event={:?}", event);

        let mut gst_buffer = gst::Buffer::with_size(event.payload.len()).unwrap();
        {
            let buffer_ref = gst_buffer.get_mut().unwrap();

            let segment = element
                .get_segment()
                .downcast::<gst::format::Time>()
                .unwrap();
            gst_log!(CAT, obj: element, "create: segment={:?}", segment);
            // If start_pts_at_zero=false (default), segment.get_time() equals 0 so the PTS will equal
            // the timestamp stored in the Pravega timestamp.
            let pts = ClockTime(event.header.timestamp.nanoseconds()) - segment.get_time();
            gst_log!(CAT, obj: element, "create: timestamp={}, pts={}, payload_len={}",
                event.header.timestamp, pts, event.payload.len());

            buffer_ref.set_pts(pts);
            if event.header.random_access {
                buffer_ref.unset_flags(gst::BufferFlags::DELTA_UNIT);
            } else {
                buffer_ref.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
            if event.header.discontinuity {
                buffer_ref.set_flags(gst::BufferFlags::DISCONT);
            }

            let mut buffer_map = buffer_ref.map_writable().unwrap();
            let slice = buffer_map.as_mut_slice();
            slice.copy_from_slice(event.payload);
        }

        Ok(gst_buffer)
    }
}
