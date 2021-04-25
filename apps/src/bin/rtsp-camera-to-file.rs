//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use clap::Clap;
use gst::prelude::*;
use log::info;

/// RTSP Camera to Pravega.
#[derive(Clap)]
struct Opts {
    // /// Pravega controller in format "127.0.0.1:9090"
    // #[clap(short, long, default_value = "127.0.0.1:9090")]
    // controller: String,
    // /// Pravega scope/stream
    // #[clap(short, long)]
    // stream: String,
    /// RTSP URL
    #[clap(long)]
    location: String,
}

fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();

    // Initialize GStreamer
    if let Err(err) = gst::init() {
        eprintln!("Failed to initialize Gst: {}", err);
        return;
    }

    let pipeline_description =
        "rtspsrc name=src".to_owned()
        + "   buffer-mode=none"                 // Outgoing timestamps are calculated directly from the RTP timestamps. This mode is good for recording.
                                                // This will provide the RTP timestamps as PTS (and the arrival timestamps as DTS).
                                                // See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
        + "   drop-messages-interval=0"         // Always log when rtp packets have been dropped
        + "   drop-on-latency=true"             // Drop oldest buffers when the queue is completely filled
        + "   latency=2000"                     // Set the maximum latency of the jitterbuffer (milliseconds).
                                                // Packets will be kept in the buffer for at most this time.
        + "   ntp-sync=true"                    // Required to get NTP timestamps as PTS
        + "   ntp-time-source=running-time"     // Required to get NTP timestamps as PTS
        // Video
        + " src."
        + " ! rtph264depay"                     // Extract H264 elementary stream
        + " ! h264parse"                        // Parse H264
        + " ! video/x-h264,alignment=au"        // Must align on Access Units for mpegtsmux
        + " ! mux."                             // Send video to muxer
        // Audio
        + " src."
        + " ! rtpmp4gdepay"                     // Extract audio
        + " ! aacparse"                         // Parse audio
        + " ! mux."                             // Send audio to muxer
        // Video + Audio Muxer
        + " mpegtsmux name=mux"                 // Packetize in MPEG transport stream
        + " ! queue"
        + " ! filesink location=/mnt/data/tmp/rtsp-camera-to-filesink.ts sync=false"
        // + " ! pravegasink name=sink"            // Write to Pravega
        // + "   timestamp-mode=ntp"               // Required to use NTP timestamps in PTS
        // + "   sync=false"                       // Always write to Pravega immediately regardless of PTS
        ;
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let clock = gst::SystemClock::obtain();
    clock.set_property("clock-type", &gst::ClockType::Realtime).unwrap();
    println!("clock={:?}, time={:?}", clock, clock.time());
    pipeline.use_clock(Some(&clock));

    let rtspsrc = pipeline
        .clone()
        .dynamic_cast::<gst::Pipeline>().unwrap()
        .get_by_name("src").unwrap();
    rtspsrc.set_property("location", &opts.location).unwrap();

    // let pravegasink = pipeline.get_by_name("sink").unwrap();
    // pravegasink.set_property("controller", &opts.controller).unwrap();
    // pravegasink.set_property("stream", &opts.stream).unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    // Shutdown pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
