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
    use pravega_video::timestamp::{PravegaTimestamp, MSECOND, NSECOND};
    use rstest::rstest;
    use std::convert::TryFrom;
    use std::time::Instant;
    #[allow(unused_imports)]
    use tracing::{error, info, debug, trace};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    fn pravega_src_test_data_gen(test_config: &TestConfig, stream_name: &str) -> Result<BufferListSummary, Error> {
        gst_init();
        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={:?}", first_timestamp);
        let fps = 30;
        let key_int_max = 30;
        let length_sec = 5;
        let num_buffers_written = length_sec * fps;

        // We write an MP4 stream without fragmp4pay because the first few buffers have no timestamp and will not be indexed.
        // This allows us to distinguish between starting at the first buffer in the data stream vs. the first indexed buffer.
        // The tests in this module do not decode the video so the encoder and container are not significant.
        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! x264enc key-int-max={key_int_max} bitrate=100 \
            ! mp4mux streamable=true fragment-duration=100 \
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

    // With no-seek, the segment has 0 for all times because the initial PTS is unknown. sync=true cannot be used.
    #[rstest]
    #[case(false)]
    fn test_pravegasrc_start_mode_no_seek(#[case] sync: bool) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        info!("#### Read video stream");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync={sync}",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            sync = sync,
        );
        let t0 = Instant::now();
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={}", summary);
        let wallclock_elapsed_time = (Instant::now() - t0).as_nanos() * NSECOND;
        debug!("wallclock_elapsed_time={}", wallclock_elapsed_time);
        info!("Expected: summary={:?}", summary_written);
        info!("Actual:   summary={:?}", summary);
        assert_eq!(summary, summary_written);
        assert!(summary.buffer_summary_list.first().unwrap().flags.contains(gst::BufferFlags::DISCONT));
        if sync {
            assert!(wallclock_elapsed_time >= summary.pts_range());
        }
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_pravegasrc_start_mode_earliest(#[case] sync: bool) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        let first_valid_pts_written = summary_written.first_valid_pts();
        info!("#### Read video stream");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! appsink name=sink sync={sync}",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            sync = sync,
        );
        let t0 = Instant::now();
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        let wallclock_elapsed_time = (Instant::now() - t0).as_nanos() * NSECOND;
        debug!("wallclock_elapsed_time={}", wallclock_elapsed_time);
        debug!("summary={}", summary);
        let first_pts = summary.first_pts();
        assert_timestamp_eq("first_pts", first_pts, first_valid_pts_written);
        if sync {
            assert!(wallclock_elapsed_time >= summary.pts_range());
        }
    }

    #[rstest]
    #[case(0, 0, false)]
    #[case(0, 0, true)]
    #[case(2, 0, false)]
    #[case(2, 0, true)]
    #[case(2, 500, false)]
    #[case(2, 500, true)]
    #[case(usize::MAX, 0, false)]      // last non-delta record
    fn test_pravegasrc_start_mode_timestamp(#[case] start_index: usize, #[case] start_offset_ms: u64, #[case] sync: bool) {
        info!("start_index={}, start_offset_ms={}", start_index, start_offset_ms);
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        let non_delta_pts = summary_written.non_delta_pts();
        info!("non_delta_pts={:?}", non_delta_pts);
        info!("#### Read video stream");
        let start_index = std::cmp::min(start_index, non_delta_pts.len() - 1);
        let start_pts_expected = non_delta_pts[start_index];
        // We should get the same first PTS even if we specify a PTS beyond the indexed frame (but before the next one).
        let start_timestamp = start_pts_expected + start_offset_ms * MSECOND;
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=timestamp \
              start-timestamp={start_timestamp} \
            ! appsink name=sink sync={sync}",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            start_timestamp = start_timestamp.nanoseconds().unwrap(),
            sync = sync,
        );
        let t0 = Instant::now();
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        let wallclock_elapsed_time = (Instant::now() - t0).as_nanos() * NSECOND;
        debug!("wallclock_elapsed_time={}", wallclock_elapsed_time);
        debug!("summary_written={:?}", summary_written);
        debug!("summary=        {:?}", summary);
        assert_timestamp_eq("first_pts", summary.first_pts(), start_pts_expected);
        if sync {
            assert!(wallclock_elapsed_time >= summary.pts_range());
        }
    }

    #[test]
    fn test_pravegasrc_start_mode_timestamp_max() {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let _ = pravega_src_test_data_gen(test_config, stream_name).unwrap();
        info!("#### Read video stream from max timestamp");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=timestamp \
              start-timestamp={start_timestamp} \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            start_timestamp = PravegaTimestamp::MAX.nanoseconds().unwrap(),
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        assert_eq!(summary.num_buffers(), 0);
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
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={}", summary);
        assert_eq!(summary.num_buffers(), 0);
    }
}
