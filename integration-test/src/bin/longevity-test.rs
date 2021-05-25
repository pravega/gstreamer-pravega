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
pub const DEFAULT_RUST_LOG: &str = "longevity_test=debug,warn";

/// Pravega video player.
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "tcp://127.0.0.1:9090"
    #[clap(short, long, default_value = "tcp://127.0.0.1:9090")]
    controller: String,
    /// The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.
    #[clap(short, long)]
    keycloak_file: Option<String>,
    /// Pravega scope/stream
    #[clap(short, long)]
    stream: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamingDecodedBufferValidator {
    stream: String,
    max_gap: TimeDelta,
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

impl StreamingDecodedBufferValidator {
    pub fn new(stream: &str, max_gap: TimeDelta) -> StreamingDecodedBufferValidator{
        StreamingDecodedBufferValidator {
            stream: stream.to_owned(),
            max_gap: max_gap,
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
        self.buffer_count += 1;
        if pts.is_none() {
            event!(Level::WARN, description = "PTS is missing",
                prev_pts = ?self.prev_pts,
                offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
            self.pts_missing_count += 1;
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
                    if time_delta > self.max_gap {
                        event!(Level::WARN, description = "Gap in PTS is too large",
                            time_delta = ?time_delta, prev_pts = ?self.prev_pts,
                            pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
                        self.pts_gap_too_large_count += 1;
                    }
                    self.prev_pts = pts;
                } else {
                    event!(Level::WARN, description = "PTS is decreasing",
                        time_delta = ?time_delta, prev_pts = ?self.prev_pts,
                        pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
                    self.pts_decreasing_count += 1;
                }
            }
        }
        if flags.contains(gst::BufferFlags::DISCONT) {
            event!(Level::WARN, description = "discontinuity",
                pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
            self.discontinuity_count += 1;
        }
        if flags.contains(gst::BufferFlags::CORRUPTED) {
            event!(Level::WARN, description = "corrupted",
                pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
            self.corrupted_count += 1;
        }
    }

    pub fn log_stats(&self) {
        event!(Level::INFO, description = "statistics",
            buffer_count = self.buffer_count,
            min_pts = ?self.min_pts,
            max_pts = ?self.max_pts,
            pts_range = ?self.max_pts - self.min_pts,
            pts_missing_count = self.pts_missing_count,
            pts_gap_too_large_count = self.pts_gap_too_large_count,
            pts_decreasing_count = self.pts_decreasing_count,
            discontinuity_count = self.discontinuity_count,
            corrupted_count = self.corrupted_count,
            stream = %self.stream);
    }
}

fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| DEFAULT_RUST_LOG.to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    match std::env::var("GST_DEBUG") {
        Ok(_) => (),
        Err(_) => std::env::set_var("GST_DEBUG", DEFAULT_GST_DEBUG),
    };

    gst::init()?;
    gstpravega::plugin_register_static().unwrap();
    let main_loop = glib::MainLoop::new(None, false);

    let pipeline_description = format!(
        "pravegasrc name=src \
          start-mode=earliest \
        ! decodebin \
        ! appsink name=sink sync=false"
    );
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let pravegasrc = pipeline.clone().dynamic_cast::<gst::Pipeline>().unwrap().by_name("src").unwrap();
    pravegasrc.set_property("controller", &opts.controller).unwrap();
    pravegasrc.set_property("stream", &opts.stream).unwrap();
    pravegasrc.set_property("keycloak-file", &opts.keycloak_file.unwrap()).unwrap();
    pravegasrc.set_property("allow-create-scope", &false).unwrap();

    let max_gap = 100 * MSECOND;

    let validator = Arc::new(Mutex::new(
        StreamingDecodedBufferValidator::new(
            &opts.stream[..],
            max_gap,
    )));

    let validator_clone = validator.clone();
    let sink = pipeline.by_name("sink");
    match sink {
        Some(sink) => {
            let sink = sink.downcast::<gst_app::AppSink>().unwrap();
            sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().unwrap();
                        trace!("sample={:?}", sample);
                        let buffer = sample.buffer().unwrap();
                        let mut validator = validator_clone.lock().unwrap();
                        validator.record_buffer(buffer);
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build()
            );
        },
        None => warn!("Element named 'sink' not found"),
    };

    let validator_clone = validator.clone();
    let timeout_id = glib::timeout_add(std::time::Duration::from_secs(60), move || {
        let validator = validator_clone.lock().unwrap();
        validator.log_stats();
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
