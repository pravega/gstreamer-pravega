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
    use pravega_video::timestamp::{PravegaTimestamp, MSECOND, SECOND};
    use rstest::rstest;
    use std::convert::TryFrom;
    use std::sync::Arc;
    use std::time::Instant;
    #[allow(unused_imports)]
    use tracing::{error, info, debug, trace};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    fn pravegasrc_seek_test_data_gen(test_config: &TestConfig, stream_name: &str, video_encoder: VideoEncoder, container_format: ContainerFormat) -> Result<BufferListSummary, Error> {
        gst_init();
        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={:?}", first_timestamp);
        let fps = 30;
        let length_sec = 60;
        let num_buffers_written = length_sec * fps;
        let video_encoder_pipeline = video_encoder.pipeline();
        let container_pipeline = container_format.pipeline();

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! {video_encoder_pipeline} \
            ! {container_pipeline} \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_timestamp.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={}", summary);
        Ok(summary)
    }

    /// Test seeking that occurs in Pravega Video Player.
    /// This starts playback from the beginning, with sync=true, then skips over several seconds.
    /// Based on https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/master/tutorials/src/bin/basic-tutorial-4.rs
    #[rstest]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(500 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(200 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::MpegTs,
    )]
    fn test_pravegasrc_seek_player(#[case] video_encoder: VideoEncoder, #[case] container_format: ContainerFormat) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravegasrc_seek_test_data_gen(test_config, stream_name, video_encoder, container_format).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();
        let last_pts_written = summary_written.last_valid_pts();

        info!("#### Read video stream without decoding");
        info!("### Build pipeline");
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! identity silent=false \
            ! appsink name=sink \
              sync=true",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );

        info!("Launch Pipeline: {}", pipeline_description);
        let pipeline = gst::parse_launch(&pipeline_description).unwrap();
        let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
        // Subscribe to any property changes.
        // Identity elements with silent=false will produce bus messages and will be logged by monitor_pipeline_until_eos.
        let _ = pipeline.add_property_deep_notify_watch(None, true);
        let summary_list = Arc::new(Mutex::new(Vec::new()));
        let summary_list_clone = summary_list.clone();
        let sink = pipeline
            .by_name("sink");
        match sink {
            Some(sink) => {
                let sink = sink.downcast::<gst_app::AppSink>().unwrap();
                sink.set_callbacks(
                    gst_app::AppSinkCallbacks::builder()
                        .new_sample(move |sink| {
                            let sample = sink.pull_sample().unwrap();
                            debug!("sample={:?}", sample);
                            let buffer = sample.buffer().unwrap();
                            let summary = BufferSummary::from(buffer);
                            let mut summary_list = summary_list_clone.lock().unwrap();
                            summary_list.push(summary);
                            Ok(gst::FlowSuccess::Ok)
                        })
                        .build()
                );
            },
            None => warn!("Element named 'sink' not found"),
        };

        info!("### Play pipeline");
        pipeline.set_state(gst::State::Playing).unwrap();
        info!("current_state={:?}", pipeline.current_state());

        let seek_at_pts = first_pts_written + 10 * SECOND;
        let seek_to_pts = first_pts_written + 50 * SECOND;
        debug!("first_pts_written={:?}", first_pts_written);
        debug!("seek_at_pts=      {:?}", seek_at_pts);
        debug!("seek_to_pts=      {:?}", seek_to_pts);

        let mut last_query_time = Instant::now();

        let bus = pipeline.bus().unwrap();
        loop {
            let msg = bus.timed_pop(100 * gst::MSECOND);
            trace!("Bus message: {:?}", msg);

            // Query the current position (pts) every 100 ms.
            // Perform the seek at the desired pts.
            let now = Instant::now();
            if (now - last_query_time).as_millis() > 100 {
                if let Some(position) = pipeline.query_position::<gst::ClockTime>() {
                    info!("position={}", position);
                    let timestamp = clocktime_to_pravega(position);
                    if seek_at_pts <= timestamp && timestamp < seek_to_pts {
                        info!("Performing seek");
                        pipeline.seek_simple(
                                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                pravega_to_clocktime(seek_to_pts),
                        ).unwrap();
                    }
                }
                last_query_time = now;
            }

            match msg {
                Some(msg) => {
                    match msg.view() {
                        gst::MessageView::Eos(..) => break,
                        gst::MessageView::Error(err) => {
                            let msg = format!(
                                "Error from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                            let _ = pipeline.set_state(gst::State::Null);
                            panic!("msg={}", msg);
                        },
                        gst::MessageView::PropertyNotify(p) => {
                            // Identity elements with silent=false will produce this message after watching with `pipeline.add_property_deep_notify_watch(None, true)`.
                            let (_, property_name, value) = p.get();
                            match value {
                                Some(value) => match value.get::<String>() {
                                    Ok(value) => if !value.is_empty() {
                                        debug!("PropertyNotify: {}={}", property_name, value);
                                    },
                                    _ => {}
                                },
                                _ => (),
                            };
                        },
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
        let first_pts_read = summary.first_valid_pts();
        let last_pts_read = summary.last_valid_pts();
        assert_between_timestamp("first_pts_read", first_pts_read, first_pts_written - 1 * SECOND, first_pts_written + 1 * SECOND);
        assert_between_timestamp("last_pts_read", last_pts_read, last_pts_written - 1 * SECOND, last_pts_written + 1 * SECOND);
        // Confirm there are no buffers that should have been skipped.
        assert_eq!(summary.buffers_between(seek_at_pts + 10 * SECOND, seek_to_pts - 10 * SECOND).len(), 0);
    }

    /// This will test starting a decode pipeline at a precise time.
    /// The pipeline should start decoding at the random access point prior to the specified timestamp
    /// but the decoder should not emit frames earlier than the specified timestamp.
    /// See also test_pravegasrc_start_mode_timestamp().
    #[rstest]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(500 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(200 * MSECOND).build().unwrap()),
    )]
    fn test_pravegasrc_decode_from_timestamp(#[case] video_encoder: VideoEncoder, #[case] container_format: ContainerFormat) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravegasrc_seek_test_data_gen(test_config, stream_name, video_encoder, container_format).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();
        let last_pts_written = summary_written.last_valid_pts();

        info!("#### Decode entire video stream");
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_full = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written={}", summary_written);
        debug!("summary_full=   {}", summary_full);
        let first_pts_full = summary_full.first_valid_pts();
        let last_pts_full = summary_full.last_valid_pts();

        info!("#### Decode video stream starting from exact PTS");
        let resume_from_pts: PravegaTimestamp = first_pts_full + 30510 * MSECOND;
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=timestamp \
              start-timestamp={resume_from_pts} \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            resume_from_pts = resume_from_pts.nanoseconds().unwrap(),
        );

        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written={}", summary_written);
        debug!("summary_full=   {}", summary_full);
        debug!("summary=        {}", summary);

        let first_pts_read = summary.first_valid_pts();
        let last_pts_read = summary.last_valid_pts();
        debug!("first_pts_written={:?}", first_pts_written);
        debug!("first_pts_full=   {:?}", first_pts_full);
        debug!("resume_from_pts=  {:?}", resume_from_pts);
        debug!("first_pts_read=   {:?}", first_pts_read);
        debug!("last_pts_written= {:?}", last_pts_written);
        debug!("last_pts_full=    {:?}", last_pts_full);
        debug!("last_pts_read=    {:?}", last_pts_read);
        assert_timestamp_eq("first_pts_read", first_pts_read, resume_from_pts);
        assert_timestamp_eq("last_pts_read", last_pts_read, last_pts_full);
    }
}
