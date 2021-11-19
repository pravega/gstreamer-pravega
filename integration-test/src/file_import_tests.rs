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
    use pravega_video::timestamp::MSECOND;
    use rstest::rstest;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    #[rstest]
    #[case(
        "default",
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    fn file_import_test(#[case] description: &str, #[case] video_encoder: VideoEncoder,
        #[case] container_format: ContainerFormat) {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        let mp4_filename = "/tmp/timestampcvt_test1.mp4";
        let start_utc = "2001-02-03T04:00:00.000Z".to_owned();
        gst_init();
        let stream_name = &format!("test-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let video_encoder_pipeline = video_encoder.pipeline();
        let container_pipeline = container_format.pipeline();

        info!("#### Write MP4 file");
        let pipeline_description = format!("\
            videotestsrc name=src num-buffers=300 \
            ! video/x-raw,width=160,height=120,framerate=30/1 \
            ! videoconvert \
            ! x264enc key-int-max=60 tune=zerolatency \
            ! mp4mux \
            ! identity silent=false \
            ! filesink location={mp4_filename}",
            mp4_filename = mp4_filename,
        );        
        launch_pipeline(&pipeline_description).unwrap();

        info!("#### Read MP4 file");
        let pipeline_description = format!("\
            uridecodebin name=src uri=file://{mp4_filename} \
            ! timestampcvt input-timestamp-mode=start-at-fixed-time start-utc={start_utc} \
            ! identity silent=false \
            ! appsink name=sink sync=false",
            mp4_filename = mp4_filename,
            start_utc = start_utc,
        );
        let summary_read_mp4 = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_read_mp4={}", summary_read_mp4);

        info!("#### Transcode MP4 file");
        let pipeline_description = format!("\
            uridecodebin name=src uri=file://{mp4_filename} \
            ! timestampcvt input-timestamp-mode=start-at-fixed-time start-utc={start_utc} \
            ! identity silent=false \
            ! {video_encoder_pipeline} \
            ! {container_pipeline} \
            ! appsink name=sink sync=false",
            mp4_filename = mp4_filename,
            start_utc = start_utc,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        let summary_transcode = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_transcode={}", summary_transcode);

        info!("#### Copy MP4 file to Pravega");
        let pipeline_description = format!("\
            uridecodebin name=src uri=file://{mp4_filename} \
            ! timestampcvt input-timestamp-mode=start-at-fixed-time start-utc={start_utc} \
            ! identity silent=false \
            ! {video_encoder_pipeline} \
            ! {container_pipeline} \
            ! pravegasink {pravega_plugin_properties} seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            mp4_filename = mp4_filename,
            start_utc = start_utc,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        launch_pipeline(&pipeline_description).unwrap();

        info!("#### Read from Pravega with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! decodebin \
            ! identity silent=false \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_read_pravega = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_read_pravega.dump("summary_read_pravega");
        info!("summary_read_mp4=    {}", summary_read_mp4);
        info!("summary_read_pravega={}", summary_read_pravega);
        assert_timestamp_eq("first_pts", summary_read_pravega.first_pts(), summary_read_mp4.first_pts());
        assert_timestamp_eq("last_valid_pts", summary_read_pravega.last_valid_pts(), summary_read_mp4.last_valid_pts());
        assert_between_u64("corrupted_buffer_count", summary_read_pravega.corrupted_buffer_count(), 0, 2);
        info!("#### END");
    }
}
