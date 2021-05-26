//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Pravega video player.
// Based on https://github.com/sdroege/gstreamer-rs/blob/master/tutorials/src/bin/basic-tutorial-5.rs

use anyhow::Error;
use clap::Clap;
use gst::prelude::*;
use gstreamer_video as gst_video;
use gst_video::prelude::*;
use glib::object::ObjectType;
use gtk::prelude::*;
use gtk::{Box, DrawingArea, Inhibit, Orientation, Window, WindowType};
use pravega_video::timestamp::PravegaTimestamp;
use std::{convert::TryInto, os::raw::c_void, time::SystemTime};
use std::process;
use std::ops;
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace, event, Level, span};
use tracing_subscriber::fmt::format::FmtSpan;

/// Default logging configuration for GStreamer and GStreamer plugins.
/// Valid levels are: none, ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
/// See [https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log].
pub const DEFAULT_GST_DEBUG: &str = "FIXME";
/// Default logging configuration for for Rust tracing.
/// Valid levels are: error, warn, info, debug, trace
pub const DEFAULT_RUST_LOG: &str = "pravega_video_player=info,warn";

/// Pravega video player.
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
    /// The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.
    #[clap(short, long, default_value = "", setting(clap::ArgSettings::AllowEmptyValues))]
    keycloak_file: String,
    /// Pravega scope/stream
    #[clap(short, long)]
    stream: String,
    /// If no-sync is set, frames will be displayed as soon as they are decoded
    #[clap(long)]
    no_sync: bool,
}

// Custom struct to keep our window reference alive
// and to store the timeout id so that we can remove
// it from the main context again later and drop the
// references it keeps inside its closures
struct AppWindow {
    main_window: Window,
    timeout_id: Option<glib::SourceId>,
}

impl ops::Deref for AppWindow {
    type Target = Window;

    fn deref(&self) -> &Window {
        &self.main_window
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Some(source_id) = self.timeout_id.take() {
            glib::source_remove(source_id);
        }
    }
}

// nanos_since_epoch is the number of nanoseconds since the TAI epoch.
fn format_nanos_since_epoch(nanos_since_epoch: u64) -> String {
    let timestamp = PravegaTimestamp::from_nanoseconds(Some(nanos_since_epoch));
    let system_time: SystemTime = timestamp.try_into().unwrap();
    let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
    let formatted_time = datetime.format("%Y-%m-%d %T.%3f");
    formatted_time.to_string()
}

// This creates all the GTK+ widgets that compose our application, and registers the callbacks.
fn create_ui(playbin: &gst::Pipeline, video_sink: &gst::Element) -> AppWindow {
    let main_window = Window::new(WindowType::Toplevel);
    main_window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    let play_button =
        gtk::Button::from_icon_name(Some("media-playback-start"), gtk::IconSize::SmallToolbar);
    let pipeline = playbin.clone();
    play_button.connect_clicked(move |_| {
        let pipeline = &pipeline;
        pipeline
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
    });

    let pause_button =
        gtk::Button::from_icon_name(Some("media-playback-pause"), gtk::IconSize::SmallToolbar);
    let pipeline = playbin.clone();
    pause_button.connect_clicked(move |_| {
        let pipeline = &pipeline;
        pipeline
            .set_state(gst::State::Paused)
            .expect("Unable to set the pipeline to the `Paused` state");
    });

    let stop_button =
        gtk::Button::from_icon_name(Some("media-playback-stop"), gtk::IconSize::SmallToolbar);
    let pipeline = playbin.clone();
    stop_button.connect_clicked(move |_| {
        let pipeline = &pipeline;
        pipeline
            .set_state(gst::State::Ready)
            .expect("Unable to set the pipeline to the `Ready` state");
    });

    let position_textview = gtk::TextView::new();
    position_textview.set_editable(false);
    let seek_range_textview = gtk::TextView::new();
    seek_range_textview.set_editable(false);

    let slider = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        0.0 as f64,
        100.0 as f64,
        1.0 as f64,
    );
    let pipeline = playbin.clone();
    let slider_update_signal_id = slider.connect_value_changed(move |slider| {
        let pipeline = &pipeline;
        let value = slider.value() as u64;
        info!("create_ui: handling slider change; value={}", value);
        if pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                value * gst::NSECOND,
            )
            .is_err()
        {
            error!("Seeking to {} failed", value);
        }
    });

    slider.set_draw_value(false);
    let pipeline = playbin.clone();
    let lslider = slider.clone();
    let lposition_textview = position_textview.clone();
    let lseek_range_textview = seek_range_textview.clone();
    // Update the UI (seekbar) periodically.
    let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(4000), move || {
        let pipeline = &pipeline;
        let lslider = &lslider;
        let lposition_textview = &lposition_textview;
        let lseek_range_textview = &lseek_range_textview;

        let span = span!(Level::INFO, "create_ui: seeking query");
        let (start, end) = span.in_scope(|| {
            let mut seeking_query = gst::query::Seeking::new(gst::Format::Time);
            match pipeline.query(&mut seeking_query) {
                true => {
                    let (_seekable, start, end) = seeking_query.result();
                    debug!("create_ui: seeking_query={:?}, start={:?}, end={:?}", seeking_query, start, end);
                    let start = match start {
                        gst::GenericFormattedValue::Time(start) => start.nanoseconds(),
                        _ => None,
                    };
                    let end = match end {
                        gst::GenericFormattedValue::Time(end) => end.nanoseconds(),
                        _ => None,
                    };
                    (start, end)
                },
                false => (None, None),
            }
        });
        let pos = pipeline
            .query_position::<gst::ClockTime>()
            .and_then(|pos| pos.nanoseconds());

        debug!("create_ui: start={:?}, end={:?}, pos={:?}", start, end, pos);

        if let (Some(start), Some(end), Some(pos)) = (start, end, pos) {
            let end = std::cmp::max(end, pos);
            let change_pos = pos < start;
            let pos = std::cmp::max(start, pos);
            info!("create_ui: start={:?}, end={:?}, pos={:?}, change_pos={}", start, end, pos, change_pos);

            let position_textbuf = lposition_textview
                .buffer()
                .expect("Couldn't get buffer from text_view");
            position_textbuf.set_text(&format!("Position: {}", format_nanos_since_epoch(pos)));

            let lseek_range_textbuf = lseek_range_textview
                .buffer()
                .expect("Couldn't get buffer from text_view");
            lseek_range_textbuf.set_text(&format!(
                "Start: {}  End: {}",
                format_nanos_since_epoch(start),
                format_nanos_since_epoch(end),
            ));

            if change_pos {
                lslider.set_range(start as f64, end as f64);
                lslider.set_value(pos as f64);
            } else {
                lslider.block_signal(&slider_update_signal_id);
                lslider.set_range(start as f64, end as f64);
                lslider.set_value(pos as f64);
                lslider.unblock_signal(&slider_update_signal_id);
            }
        }

        Continue(true)
    });

    let controls = Box::new(Orientation::Horizontal, 0);
    controls.pack_start(&play_button, false, false, 0);
    controls.pack_start(&pause_button, false, false, 0);
    controls.pack_start(&stop_button, false, false, 0);
    controls.pack_start(&slider, true, true, 2);

    let video_window = DrawingArea::new();

    let video_overlay = video_sink
        .clone()
        .dynamic_cast::<gst_video::VideoOverlay>().unwrap();

    video_window.connect_realize(move |video_window| {
        let video_overlay = &video_overlay;
        let gdk_window = video_window.window().unwrap();

        if !gdk_window.ensure_native() {
            error!("Can't create native window for widget");
            process::exit(-1);
        }

        let display_type_name = gdk_window.display().type_().name();
        #[cfg(all(target_os = "linux", feature = "x11"))]
        {
            // Check if we're using X11 or ...
            if display_type_name == "GdkX11Display" {
                extern "C" {
                    pub fn gdk_x11_window_get_xid(
                        window: *mut glib::object::GObject,
                    ) -> *mut c_void;
                }

                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    let xid = gdk_x11_window_get_xid(gdk_window.as_ptr() as *mut _);
                    video_overlay.set_window_handle(xid as usize);
                }
            } else {
                error!("Add support for display type '{}'", display_type_name);
                process::exit(-1);
            }
        }
        #[cfg(all(target_os = "macos", feature = "quartz"))]
        {
            if display_type_name == "GdkQuartzDisplay" {
                extern "C" {
                    pub fn gdk_quartz_window_get_nsview(
                        window: *mut glib::object::GObject,
                    ) -> *mut c_void;
                }

                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    let window = gdk_quartz_window_get_nsview(gdk_window.as_ptr() as *mut _);
                    video_overlay.set_window_handle(window as usize);
                }
            } else {
                info!(
                    "Unsupported display type '{}', compile with `--feature `",
                    display_type_name
                );
                process::exit(-1);
            }
        }
    });

    let vbox = Box::new(Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);

    let main_box = Box::new(Orientation::Vertical, 0);
    main_box.pack_start(&vbox, true, true, 0);
    main_box.pack_start(&seek_range_textview, false, false, 2);
    main_box.pack_start(&position_textview, false, false, 2);
    main_box.pack_start(&controls, false, false, 0);
    main_window.add(&main_box);
    main_window.set_default_size(640, 480);

    main_window.show_all();

    AppWindow {
        main_window,
        timeout_id: Some(timeout_id),
    }
}

pub fn run() {
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

    // Make sure the right features were activated
    #[allow(clippy::eq_op)]
    {
        if !cfg!(feature = "x11") && !cfg!(feature = "quartz") {
            error!(
                "No Gdk backend selected, compile with --features x11|quartz."
            );

            return;
        }
    }

    // Initialize GTK
    if let Err(err) = gtk::init() {
        error!("Failed to initialize GTK: {}", err);
        return;
    }

    // Initialize GStreamer
    if let Err(err) = gst::init() {
        error!("Failed to initialize Gst: {}", err);
        return;
    }

    let pipeline_description =
        "pravegasrc name=src".to_owned()
        + " ! queue2 use-buffering=true"
        + " ! decodebin name=decodebin"
        ;
    info!("Launch Pipeline: {}", pipeline_description);
    let playbin = gst::parse_launch(&pipeline_description.to_owned()).unwrap();

    let playbin = playbin.dynamic_cast::<gst::Pipeline>().unwrap();

    let pravegasrc = playbin
        .clone()
        .dynamic_cast::<gst::Pipeline>().unwrap()
        .by_name("src").unwrap();
    pravegasrc.set_property("controller", &opts.controller).unwrap();
    pravegasrc.set_property("stream", &opts.stream).unwrap();
    pravegasrc.set_property("keycloak-file", &opts.keycloak_file).unwrap();
    pravegasrc.set_property("allow-create-scope", &false).unwrap();

    let decodebin = playbin
        .clone()
        .dynamic_cast::<gst::Pipeline>().unwrap()
        .by_name("decodebin").unwrap();

    let video_sink = gst::ElementFactory::make("glimagesink", Some("videosink")).unwrap();
    video_sink.set_property("sync", &(!opts.no_sync)).unwrap();

    let window = create_ui(&playbin, &video_sink);

    // decodebin handling from https://github.com/sdroege/gstreamer-rs/blob/2022890766677d697407c0931442f92c4bf954d8/examples/src/bin/decodebin.rs.
    // Need to move a new reference into the closure.
    // !!ATTENTION!!:
    // It might seem appealing to use pipeline.clone() here, because that greatly
    // simplifies the code within the callback. What this actually does, however, is creating
    // a memory leak. The clone of a pipeline is a new strong reference on the pipeline.
    // Storing this strong reference of the pipeline within the callback (we are moving it in!),
    // which is in turn stored in another strong reference on the pipeline is creating a
    // reference cycle.
    // DO NOT USE pipeline.clone() TO USE THE PIPELINE WITHIN A CALLBACK
    let pipeline_weak = playbin.downgrade();
    // Connect to decodebin's pad-added signal, that is emitted whenever
    // it found another stream from the input file and found a way to decode it to its raw format.
    // decodebin automatically adds a src-pad for this raw stream, which
    // we can use to build the follow-up pipeline.
    decodebin.connect_pad_added(move |dbin, src_pad| {
        info!("connect_pad_added: BEGIN");
        // Here we temporarily retrieve a strong reference on the pipeline from the weak one
        // we moved into this callback.
        let pipeline = match pipeline_weak.upgrade() {
            Some(pipeline) => pipeline,
            None => return,
        };

        // Try to detect whether the raw stream decodebin provided us with
        // just now is either audio or video (or none of both, e.g. subtitles).
        let (is_audio, is_video) = {
            let media_type = src_pad.current_caps().and_then(|caps| {
                caps.structure(0).map(|s| {
                    let name = s.name();
                    (name.starts_with("audio/"), name.starts_with("video/"))
                })
            });

            match media_type {
                None => {
                    gst::element_warning!(
                        dbin,
                        gst::CoreError::Negotiation,
                        ("Failed to get media type from pad {}", src_pad.name())
                    );

                    return;
                }
                Some(media_type) => media_type,
            }
        };
        info!("connect_pad_added: is_audio={}, is_video={}", is_audio, is_video);

        // We create a closure here, calling it directly below it, because this greatly
        // improves readability for error-handling. Like this, we can simply use the
        // ?-operator within the closure, and handle the actual error down below where
        // we call the insert_sink(..) closure.
        let insert_sink = |is_audio, is_video| -> Result<(), Error> {
            if is_audio {
                // decodebin found a raw audiostream, so we build the follow-up pipeline to
                // play it on the default audio playback device (using autoaudiosink).
                let queue = gst::ElementFactory::make("queue", None).unwrap();
                let convert = gst::ElementFactory::make("audioconvert", None).unwrap();
                let resample = gst::ElementFactory::make("audioresample", None).unwrap();
                let sink = gst::ElementFactory::make("autoaudiosink", None).unwrap();

                let elements = &[&queue, &convert, &resample, &sink];
                pipeline.add_many(elements)?;
                gst::Element::link_many(elements)?;

                // !!ATTENTION!!:
                // This is quite important and people forget it often. Without making sure that
                // the new elements have the same state as the pipeline, things will fail later.
                // They would still be in Null state and can't process data.
                for e in elements {
                    e.sync_state_with_parent()?;
                }

                // Get the queue element's sink pad and link the decodebin's newly created
                // src pad for the audio stream to it.
                let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                src_pad.link(&sink_pad)?;
            } else if is_video {
                // decodebin found a raw videostream, so we build the follow-up pipeline to
                // display it using the autovideosink.
                let queue = gst::ElementFactory::make("queue", None).unwrap();
                let convert = gst::ElementFactory::make("videoconvert", None).unwrap();
                let scale = gst::ElementFactory::make("videoscale", None).unwrap();

                if let Some(_) = pipeline.by_name("videosink") {
                    pipeline.remove(&video_sink)?;
                }

                let elements = &[&queue, &convert, &scale, &video_sink];
                pipeline.add_many(elements)?;
                gst::Element::link_many(elements)?;

                for e in elements {
                    e.sync_state_with_parent()?
                }

                // Get the queue element's sink pad and link the decodebin's newly created
                // src pad for the video stream to it.
                let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                src_pad.link(&sink_pad)?;
            }

            Ok(())
        };

        // When adding and linking new elements in a callback fails, error information is often sparse.
        // GStreamer's built-in debugging can be hard to link back to the exact position within the code
        // that failed. Since callbacks are called from random threads within the pipeline, it can get hard
        // to get good error information. The macros used in the following can solve that. With the use
        // of those, one can send arbitrary rust types (using the pipeline's bus) into the mainloop.
        // What we send here is unpacked down below, in the iteration-code over sent bus-messages.
        // Because we are using the failure crate for error details here, we even get a backtrace for
        // where the error was constructed. (If RUST_BACKTRACE=1 is set)
        if let Err(err) = insert_sink(is_audio, is_video) {
            gst::element_error!(
                dbin,
                gst::LibraryError::Failed,
                ("Failed to insert sink"),
                ["{}", err]
            );
        }
        info!("connect_pad_added: END");
    });

    let bus = playbin.bus().unwrap();
    bus.add_signal_watch();

    let pipeline_weak = playbin.downgrade();
    bus.connect_message(None, move |_, msg| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(pipeline) => pipeline,
            None => return,
        };

        match msg.view() {
            // This is called when an End-Of-Stream message is posted on the bus.
            // We just set the pipeline to READY (which stops playback).
            gst::MessageView::Eos(..) => {
                info!("End-Of-Stream reached.");
                pipeline
                    .set_state(gst::State::Ready)
                    .expect("Unable to set the pipeline to the `Ready` state");
            }

            // This is called when an error message is posted on the bus
            gst::MessageView::Error(err) => {
                info!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            // This is called when the pipeline changes states. We use it to
            // keep track of the current state.
            gst::MessageView::StateChanged(state_changed) => {
                if state_changed
                    .src()
                    .map(|s| s == pipeline)
                    .unwrap_or(false)
                {
                    info!("State set to {:?}", state_changed.current());
                }
            }
            _ => (),
        }
    });

    playbin
        .set_state(gst::State::Playing)
        .expect("Unable to set the playbin to the `Playing` state");

    gtk::main();
    window.hide();
    playbin
        .set_state(gst::State::Null)
        .expect("Unable to set the playbin to the `Null` state");

    bus.remove_signal_watch();
}

fn main() {
    run();
    info!("main: END");
}
