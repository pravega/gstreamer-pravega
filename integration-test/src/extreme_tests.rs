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
    use pravega_video::timestamp::{SECOND, MSECOND};
    use rstest::rstest;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    #[rstest]
    #[case(
        "default",
        VideoSource::VideoTestSrc(VideoTestSrcConfigBuilder::default().build().unwrap()),
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        "maximum PTS",
        VideoSource::VideoTestSrc(VideoTestSrcConfigBuilder::default()
            .first_utc("2262-02-03T04:00:00.000Z".to_owned())
            .build().unwrap()),
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        "MP4 fragment greater than 8 MiB",
        VideoSource::VideoTestSrc(VideoTestSrcConfigBuilder::default()
            .width(7680).height(4320)   // 8K UHD, 33 megapixels
            .duration(2 * SECOND)
            .build().unwrap()),
        VideoEncoder::H264(H264EncoderConfigBuilder::default()
            .bitrate_kilobytes_per_sec(2_048_000.0 / 8.0)   // 2 Gbps, which is the maximum rate allowed by x264enc
            .key_int_max_frames(10)
            .build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default()
            .fragment_duration(200 * MSECOND)   // Each MP4 fragment will hold 2 frames.
            .build().unwrap()),
    )]
    fn test_extreme(#[case] description: &str, #[case] video_source: VideoSource, #[case] video_encoder: VideoEncoder,
            #[case] container_format: ContainerFormat) {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        info!("description={:?}", description);
        info!("video_source={:?}", video_source);
        info!("video_encoder={:?}", video_encoder);
        info!("container_format={:?}", container_format);
        gst_init();
        let stream_name = &format!("test-extreme-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let video_source_pipeline = video_source.pipeline();
        let video_encoder_pipeline = video_encoder.pipeline();
        let container_pipeline = container_format.pipeline();

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!("\
            {video_source_pipeline} \
            ! identity silent=false \
            ! tee name=t \
            t. ! queue max-size-buffers=0 max-size-bytes=0 max-size-time=0 ! appsink name=sink sync=false \
            t. ! queue \
               ! {video_encoder_pipeline} \
               ! {container_pipeline} \
               ! pravegasink {pravega_plugin_properties} seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            video_source_pipeline = video_source_pipeline,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        let summary_raw = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_raw.dump("summary_raw");
        debug!("summary_raw={}", summary_raw);

        info!("#### Read video stream without decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_read = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_read={}", summary_read);
        summary_read.dump("summary_read");

        info!("#### Read video stream with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! decodebin \
            ! identity silent=false \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_decoded = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_decoded.dump("summary_decoded");
        info!("summary_read=             {}", summary_read);
        info!("Expected: summary_raw=    {}", summary_raw);
        info!("Actual:   summary_decoded={}", summary_decoded);
        assert_eq!(summary_raw, summary_decoded);
        assert_between_u64("corrupted_buffer_count", summary_decoded.corrupted_buffer_count(), 0, 2);
        info!("#### END");
    }
}
