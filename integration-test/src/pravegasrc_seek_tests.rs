//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#[cfg(test)]
mod test {
    use anyhow::Error;
    use gst::prelude::*;
    use gstpravega::utils::{clocktime_to_pravega, pravega_to_clocktime};
    use pravega_video::timestamp::PravegaTimestamp;
    use std::convert::TryFrom;
    use std::sync::Arc;
    use std::time::Instant;
    #[allow(unused_imports)]
    use tracing::{error, info, debug, trace};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    fn pravegasrc_seek_test_data_gen(test_config: &TestConfig, stream_name: &str) -> Result<BufferListSummary, Error> {
        gst_init();
        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={:?}", first_timestamp);
        let fps = 30;
        let key_int_max = 30;
        let length_sec = 60;
        let num_buffers_written = length_sec * fps;

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! x264enc key-int-max={key_int_max} bitrate=100 \
            ! mpegtsmux \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_timestamp.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            key_int_max = key_int_max,
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={}", summary);
        Ok(summary)
    }

    /// Based on https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/master/tutorials/src/bin/basic-tutorial-4.rs
    #[test]
    fn test_pravegasrc_seek_sync() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravegasrc_seek_test_data_gen(test_config, stream_name).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();

        info!("#### Read video stream without decoding");
        info!("### Build pipeline");
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! identity silent=false \
            ! appsink name=sink \
              sync=true",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );

        let seek_at_pts = clocktime_to_pravega(pravega_to_clocktime(first_pts_written) + 1 * gst::SECOND);
        let seek_to_pts = clocktime_to_pravega(pravega_to_clocktime(seek_at_pts) + 1 * gst::SECOND);
        debug!("first_pts_written={:?}", first_pts_written);
        debug!("seek_at_pts=      {:?}", seek_at_pts);
        debug!("seek_to_pts=      {:?}", seek_to_pts);

        info!("Launch Pipeline: {}", pipeline_description);
        let pipeline = gst::parse_launch(&pipeline_description).unwrap();
        let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
        // Subscribe to any property changes.
        // Identity elements with silent=false will produce bus messages and will be logged by monitor_pipeline_until_eos.
        let _ = pipeline.add_property_deep_notify_watch(None, true);
        let summary_list = Arc::new(Mutex::new(Vec::new()));
        let summary_list_clone = summary_list.clone();
        let sink = pipeline
            .get_by_name("sink");
        match sink {
            Some(sink) => {
                let sink = sink.downcast::<gst_app::AppSink>().unwrap();
                sink.set_callbacks(
                    gst_app::AppSinkCallbacks::builder()
                        .new_sample(move |sink| {
                            let sample = sink.pull_sample().unwrap();
                            debug!("sample={:?}", sample);
                            let buffer = sample.get_buffer().unwrap();
                            let pts = clocktime_to_pravega(buffer.get_pts());
                            let summary = BufferSummary {
                                pts,
                                size: buffer.get_size() as u64,
                                offset: buffer.get_offset(),
                                offset_end: buffer.get_offset_end(),
                                flags: buffer.get_flags(),
                            };
                            let mut summary_list = summary_list_clone.lock().unwrap();
                            summary_list.push(summary);
                            Ok(gst::FlowSuccess::Ok)
                        })
                        .build()
                );
            },
            None => warn!("Element named 'sink' not found"),
        };

        info!("current_state={:?}", pipeline.get_current_state());

        // We must changed to Paused before seeking.
        info!("### Change state to Paused");
        pipeline.set_state(gst::State::Paused).unwrap();
        info!("current_state={:?}", pipeline.get_current_state());

        info!("### Sleeping while paused");
        std::thread::sleep(std::time::Duration::from_secs(3));
        info!("current_state={:?}", pipeline.get_current_state());

        info!("### Seeking to first pts");
        pipeline.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                pravega_to_clocktime(first_pts_written),
        ).unwrap();

        info!("### Play pipeline");
        pipeline.set_state(gst::State::Playing).unwrap();
        info!("current_state={:?}", pipeline.get_current_state());

        let mut last_query_time = Instant::now();

        let bus = pipeline.get_bus().unwrap();
        loop {
            let msg = bus.timed_pop(100 * gst::MSECOND);
            trace!("Bus message: {:?}", msg);

            let now = Instant::now();
            if (now - last_query_time).as_millis() > 1000 {
                let position = pipeline.query_position::<gst::ClockTime>().unwrap();
                info!("position={}", position);
                // if 10 * gst::SECOND < position && position < 30 * gst::SECOND {
                //     info!("Performing seek");
                //     pipeline.seek_simple(
                //             gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                //             30 * gst::SECOND,
                //     ).unwrap();
                // }
                last_query_time = now;
            }

            match msg {
                Some(msg) => {
                    match msg.view() {
                        gst::MessageView::Eos(..) => break,
                        gst::MessageView::Error(err) => {
                            let msg = format!(
                                "Error from {:?}: {} ({:?})",
                                err.get_src().map(|s| s.get_path_string()),
                                err.get_error(),
                                err.get_debug()
                            );
                            let _ = pipeline.set_state(gst::State::Null);
                            panic!("msg={}", msg);
                        },
                        gst::MessageView::PropertyNotify(p) => {
                            // Identity elements with silent=false will produce this message after watching with `pipeline.add_property_deep_notify_watch(None, true)`.
                            debug!("{:?}", p);
                        }
                        _ => (),
                    }
                },
                None => {}
            }
        }

        info!("### Stop pipeline");
        pipeline.set_state(gst::State::Null).unwrap();

        let summary_list = summary_list.lock().unwrap().clone();
        let summary = BufferListSummary {
            buffer_summary_list: summary_list,
        };
        debug!("summary={}", summary);
    }
}
