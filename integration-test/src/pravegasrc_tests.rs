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
    use anyhow::{anyhow, Error};
    use gst::ClockTime;
    use gst::prelude::*;
    use gstpravega::utils::{clocktime_to_pravega, pravega_to_clocktime};
    use pravega_video::timestamp::PravegaTimestamp;
    use rstest::rstest;
    use std::convert::TryFrom;
    use std::sync::Arc;
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    fn pravega_src_test_data_gen(test_config: &TestConfig, stream_name: &str) -> Result<BufferListSummary, Error> {
        gst_init();
        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={:?}", first_timestamp);
        let first_pts_written = ClockTime(first_timestamp.nanoseconds());
        info!("first_pts_written={:?}", first_pts_written);
        let fps = 30;
        let key_int_max = 30;
        let length_sec = 5;
        let num_buffers_written = length_sec * fps;

        // We write an MP4 stream because the first few buffers have no timestamp and will not be indexed.
        // This allows us to distinguish between starting at the first buffer in the data stream vs. the first indexed buffer.
        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! timeoverlay valignment=bottom font-desc=\"Sans 48px\" shaded-background=true \
            ! videoconvert \
            ! x264enc key-int-max={key_int_max} bitrate=100 \
            ! mp4mux streamable=true fragment-duration=100 \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            key_int_max = key_int_max,
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={}", summary);
        Ok(summary)
    }

    #[test]
    fn test_pravegasrc_start_mode_no_seek() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        info!("#### Read video stream");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={}", summary);
        info!("Expected: summary={}", summary_written);
        info!("Actual:   summary={}", summary);
        assert_eq!(summary, summary_written);
    }

    #[test]
    fn test_pravegasrc_start_mode_earliest() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        let first_valid_pts_written = summary_written.first_valid_pts();
        info!("#### Read video stream");
        // TODO: Should not need to use queue.
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! queue max-size-buffers=1 max-size-time=0 max-size-bytes=1000000000 \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={}", summary);
        let first_pts = summary.first_pts();
        info!("Expected: first_pts={:?}", first_valid_pts_written);
        info!("Actual:   first_pts={:?}", first_pts);
        assert_eq!(first_pts, first_valid_pts_written);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(2, 500)]
    fn test_pravegasrc_start_mode_timestamp(#[case] start_index: usize, #[case] start_offset_ms: u64) {
        info!("start_index={}, start_offset_ms={}", start_index, start_offset_ms);
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        let non_delta_pts = summary_written.non_delta_pts();
        info!("non_delta_pts={:?}", non_delta_pts);
        info!("#### Read video stream");
        let start_pts_expected = non_delta_pts[start_index];
        // We should get the same first PTS even if we specify a PTS beyond the indexed frame (but before the next one).
        let start_timestamp = clocktime_to_pravega(pravega_to_clocktime(start_pts_expected) + start_offset_ms * gst::MSECOND);
        // TODO: Should not need to use queue.
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=timestamp \
              start-timestamp={start_timestamp} \
            ! queue max-size-buffers=1 max-size-time=0 max-size-bytes=1000000000 \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            start_timestamp = start_timestamp.nanoseconds().unwrap(),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={}", summary);
        let first_pts_actual = summary.first_pts();
        info!("Expected: first_pts={:?}", start_pts_expected);
        info!("Actual:   first_pts={:?}", first_pts_actual);
        // TODO: This fails because pravegasrc starts pts at 0. "create: timestamp=2001-02-03T04:00:04.000000000Z, pts=0:00:02.000000000, payload_len=3855"
        assert_eq!(first_pts_actual, start_pts_expected);
    }

    #[test]
    fn test_pravegasrc_start_mode_latest() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let _ = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        info!("#### Read video stream");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=latest \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        // TODO: panic: gst_element_seek_simple: assertion 'seek_pos >= 0' failed
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={}", summary);
        assert!(summary.buffer_summary_list.is_empty());
    }

    /// Based on https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/master/tutorials/src/bin/basic-tutorial-4.rs
    #[test]
    fn test_pravegasrc_seek() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();

        info!("#### Read video stream");
        info!("### Build pipeline");
        // TODO: Should not need to use queue.
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
              ! queue max-size-buffers=1 max-size-time=0 max-size-bytes=1000000000 \
              ! appsink name=sink sync=false",
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
        let pipeline_clone = pipeline.clone();
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
                            let do_seek = {
                                let sample = sink.pull_sample().unwrap();
                                debug!("sample={:?}", sample);
                                let buffer = sample.get_buffer().unwrap();
                                let pts = clocktime_to_pravega(buffer.get_pts());
                                let summary = BufferSummary {
                                    pts,
                                    size: buffer.get_size() as u64,
                                    flags: buffer.get_flags(),
                                };
                                let mut summary_list = summary_list_clone.lock().unwrap();
                                summary_list.push(summary);
                                let do_seek = seek_at_pts <= pts;
                                do_seek
                            };

                            if do_seek {
                                info!("new_sample: Calling seek_simple");
                                pipeline_clone.seek_simple(
                                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                    pravega_to_clocktime(seek_to_pts),
                                ).unwrap();
                                info!("new_sample: seek_simple returned");
                            }

                            Ok(gst::FlowSuccess::Ok)
                        })
                        .build()
                );
            },
            None => warn!("Element named 'sink' not found"),
        };

        // TODO: test preroll
        // info!("### Pre-roll pipeline");
        // pipeline.set_state(gst::State::Paused).unwrap();

        info!("### Play pipeline");
        pipeline.set_state(gst::State::Playing).unwrap();

        monitor_pipeline_until_eos(&pipeline).unwrap();

        // std::thread::sleep(std::time::Duration::from_secs(3));

        info!("### Stop pipeline");
        pipeline.set_state(gst::State::Null).unwrap();

        let summary_list = summary_list.lock().unwrap().clone();
        let summary = BufferListSummary {
            buffer_summary_list: summary_list,
        };
        debug!("summary={}", summary);
    }
}
