//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow::Error;
use clap::Clap;
use derive_builder::*;
use gst::prelude::*;
use gstpravega::utils::clocktime_to_pravega;
use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, MSECOND};
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace, event, Level, span};
use tracing_subscriber::fmt::format::FmtSpan;

/// Default logging configuration for GStreamer and GStreamer plugins.
/// Valid levels are: none, ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
/// See [https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log].
pub const DEFAULT_GST_DEBUG: &str = "WARN,pravegasrc:INFO,qtdemux:ERROR";
/// Default logging configuration for for Rust tracing.
/// Valid levels are: error, warn, info, debug, trace
pub const DEFAULT_RUST_LOG: &str = "longevity_test=info,warn";

/// Pravega video player.
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "tcp://127.0.0.1:9090"
    #[clap(long, default_value = "tcp://127.0.0.1:9090")]
    controller: String,
    /// The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.
    #[clap(long)]
    keycloak_file: Option<String>,
    /// Pravega scope/stream
    #[clap(long)]
    stream: String,
    #[clap(long)]
    start_utc: Option<String>,
    #[clap(long)]
    end_utc: Option<String>,
    /// Can be mp4 or mpegts
    #[clap(long, default_value = "mp4")]
    container_format: String,
}

#[derive(Clone, Debug, PartialEq, Builder)]
pub struct StreamingBufferValidatorConfig {
    pub probe_name: String,
    pub stream: String,
    pub element: String,
    pub pad: String,
    pub max_gap: TimeDelta,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamingBufferValidator {
    config: StreamingBufferValidatorConfig,
    prev_pts: PravegaTimestamp,
    prev_pts_plus_duration: PravegaTimestamp,
    min_pts: PravegaTimestamp,
    max_pts: PravegaTimestamp,
    buffer_count: u64,
    pts_missing_count: u64,
    pts_gap_too_large_count: u64,
    pts_decreasing_count: u64,
    discontinuity_count: u64,
    corrupted_count: u64,
}

impl StreamingBufferValidator {
    pub fn new(config: StreamingBufferValidatorConfig) -> StreamingBufferValidator{
        StreamingBufferValidator {
            config: config,
            prev_pts: PravegaTimestamp::none(),
            prev_pts_plus_duration: PravegaTimestamp::none(),
            min_pts: PravegaTimestamp::none(),
            max_pts: PravegaTimestamp::none(),
            buffer_count: 0,
            pts_missing_count: 0,
            pts_gap_too_large_count: 0,
            pts_decreasing_count: 0,
            discontinuity_count: 0,
            corrupted_count: 0,
        }
    }

    pub fn record_buffer(&mut self, buffer: &gst::BufferRef) {
        let flags = buffer.flags();
        let pts = clocktime_to_pravega(buffer.pts());

        event!(Level::DEBUG, description = "buffer",
            pts = %pts, duration_ms = buffer.duration().mseconds().unwrap_or_default(),
            offset = buffer.offset(), size = buffer.size(), flags = ?flags,
            probe_name = %self.config.probe_name,
            stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);

        self.buffer_count += 1;
        let log_pts = if pts.is_none() {
            event!(Level::WARN, description = "PTS is missing",
                pts = %self.prev_pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags,
                probe_name = %self.config.probe_name,
                stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);
            self.pts_missing_count += 1;
            self.prev_pts
        } else {
            if self.min_pts.is_none() || self.min_pts > pts {
                self.min_pts = pts;
            }
            if self.max_pts.is_none() || self.max_pts < pts {
                self.max_pts = pts;
            }
            if self.prev_pts.is_none() {
                self.prev_pts = pts;
            } else {
                let time_delta = pts - self.prev_pts;
                if time_delta >= 0 * MSECOND {
                    if time_delta > self.config.max_gap {
                        event!(Level::WARN, description = "Gap in PTS is too large",
                            time_delta = %time_delta, prev_pts = %self.prev_pts,
                            pts = %pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags,
                            stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);
                        self.pts_gap_too_large_count += 1;
                    }
                    self.prev_pts = pts;
                } else {
                    event!(Level::WARN, description = "PTS is decreasing",
                        time_delta = %time_delta, prev_pts = %self.prev_pts,
                        pts = %pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags,
                        probe_name = %self.config.probe_name,
                        stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);
                    self.pts_decreasing_count += 1;
                    self.prev_pts = pts;
                }
            }
            pts
        };
        if flags.contains(gst::BufferFlags::DISCONT) {
            event!(Level::WARN, description = "discontinuity",
                pts = %log_pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags,
                probe_name = %self.config.probe_name,
                stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);
            self.discontinuity_count += 1;
        }
        if flags.contains(gst::BufferFlags::CORRUPTED) {
            event!(Level::WARN, description = "corrupted",
                pts = %log_pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags,
                probe_name = %self.config.probe_name,
                stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad);
            self.corrupted_count += 1;
        }
    }

    pub fn log_stats(&self) {
        event!(Level::INFO, description = "statistics",
            buffer_count = self.buffer_count,
            min_pts = %self.min_pts,
            max_pts = %self.max_pts,
            pts_range = ?self.max_pts - self.min_pts,
            pts_missing_count = self.pts_missing_count,
            pts_gap_too_large_count = self.pts_gap_too_large_count,
            pts_decreasing_count = self.pts_decreasing_count,
            discontinuity_count = self.discontinuity_count,
            corrupted_count = self.corrupted_count,
            probe_name = %self.config.probe_name,
            stream = %self.config.stream, element = %self.config.element, pad = %self.config.pad,
        );
    }
}

fn install_validator(pipeline: &gst::Pipeline, config: StreamingBufferValidatorConfig) -> Arc<Mutex<StreamingBufferValidator>> {
    let validator = Arc::new(Mutex::new(StreamingBufferValidator::new(config.clone())));
    let validator_clone = validator.clone();
    let element = pipeline.by_name(config.element.as_str()).unwrap();
    let pad = element.static_pad(config.pad.as_str()).unwrap();
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, probe_info| {
        if let Some(gst::PadProbeData::Buffer(ref buffer)) = probe_info.data {
            let mut validator = validator_clone.lock().unwrap();
            validator.record_buffer(buffer);
        }
        gst::PadProbeReturn::Ok
    });
    validator
}

fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| DEFAULT_RUST_LOG.to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .json()
        .init();

    match std::env::var("GST_DEBUG") {
        Ok(_) => (),
        Err(_) => std::env::set_var("GST_DEBUG", DEFAULT_GST_DEBUG),
    };

    gst::init()?;
    gstpravega::plugin_register_static().unwrap();
    let main_loop = glib::MainLoop::new(None, false);

    let demux_pipeline = match opts.container_format.as_str() {
        "mp4" => format!("qtdemux"),
        "mpegts" => format!("tsdemux"),
        _ => panic!("Unsupported container format"),
    };

    let pipeline_description = format!(
        "pravegasrc name=pravegasrc \
        ! {demux_pipeline} \
        ! h264parse name=h264parse \
        ! avdec_h264 name=avdec_h264 \
        ! fakesink name=sink sync=false",
        demux_pipeline = demux_pipeline,
    );
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let pravegasrc = pipeline.clone().dynamic_cast::<gst::Pipeline>().unwrap().by_name("pravegasrc").unwrap();
    pravegasrc.set_property("buffer-size", 10*1024*1024 as u32).unwrap();
    pravegasrc.set_property("controller", &opts.controller).unwrap();
    pravegasrc.set_property("stream", &opts.stream).unwrap();
    pravegasrc.set_property("keycloak-file", &opts.keycloak_file.unwrap_or_default()).unwrap();
    pravegasrc.set_property("allow-create-scope", &false).unwrap();
    if let Some(start_utc) = opts.start_utc {
        pravegasrc.set_property_from_str("start-mode", "timestamp");
        pravegasrc.set_property("start-utc", &start_utc).unwrap();
    }
    if let Some(end_utc) = opts.end_utc {
        pravegasrc.set_property_from_str("end-mode", "timestamp");
        pravegasrc.set_property("end-utc", &end_utc).unwrap();
    }

    let max_gap = 100 * MSECOND;

    let pravegasrc_validator = install_validator(&pipeline,
        StreamingBufferValidatorConfigBuilder::default()
        .probe_name("1-pravegasrc".to_owned())
        .stream(opts.stream.clone())
        .element("pravegasrc".to_owned())
        .pad("src".to_owned())
        .max_gap(max_gap)
        .build().unwrap());

    let demux_validator = install_validator(&pipeline,
        StreamingBufferValidatorConfigBuilder::default()
        .probe_name("2-demux".to_owned())
        .stream(opts.stream.clone())
        .element("h264parse".to_owned())
        .pad("sink".to_owned())
        .max_gap(max_gap)
        .build().unwrap());

    let parse_validator = install_validator(&pipeline,
        StreamingBufferValidatorConfigBuilder::default()
        .probe_name("3-parse".to_owned())
        .stream(opts.stream.clone())
        .element("h264parse".to_owned())
        .pad("src".to_owned())
        .max_gap(max_gap)
        .build().unwrap());

    let decoded_validator = install_validator(&pipeline,
        StreamingBufferValidatorConfigBuilder::default()
        .probe_name("4-decode".to_owned())
        .stream(opts.stream.clone())
        .element("sink".to_owned())
        .pad("sink".to_owned())
        .max_gap(max_gap)
        .build().unwrap());

    let timeout_id = glib::timeout_add(std::time::Duration::from_secs(60), move || {
        let pravegasrc_validator = pravegasrc_validator.lock().unwrap();
        pravegasrc_validator.log_stats();
        drop(pravegasrc_validator);

        let demux_validator = demux_validator.lock().unwrap();
        demux_validator.log_stats();
        drop(demux_validator);

        let parse_validator = parse_validator.lock().unwrap();
        parse_validator.log_stats();
        drop(parse_validator);

        let decoded_validator = decoded_validator.lock().unwrap();
        decoded_validator.log_stats();
        drop(decoded_validator);
        glib::Continue(true)
    });

    let bus = pipeline.bus().unwrap();
    pipeline.set_state(gst::State::Playing)?;
    let main_loop_clone = main_loop.clone();
    bus.add_watch(move |_, msg| {
        let main_loop = &main_loop_clone;
        match msg.view() {
            gst::MessageView::Eos(..) => main_loop.quit(),
            gst::MessageView::Error(err) => {
                error!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                main_loop.quit();
            },
            _ => (),
        };
        glib::Continue(true)
    })
    .expect("Failed to add bus watch");

    main_loop.run();

    pipeline.set_state(gst::State::Null)?;
    bus.remove_watch().unwrap();
    glib::source_remove(timeout_id);
    info!("main: END");
    Ok(())
}
