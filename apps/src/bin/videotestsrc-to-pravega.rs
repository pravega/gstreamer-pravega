use gst::prelude::*;

fn main() {
    // std::env::set_var("GST_DEBUG", "pravegasink:6,basesink:6,mpegtsbase:6,mpegtspacketizer:6");

    // Initialize GStreamer
    gst::init().unwrap();

    gstpravega::plugin_register_static().unwrap();

    // This creates a pipeline by parsing the gst-launch pipeline syntax.
    let pipeline = gst::parse_launch(
        "videotestsrc name=src is-live=true do-timestamp=true num-buffers=1 \
        ! video/x-raw,width=320,height=180,framerate=30/1 \
        ! videoconvert \
        ! x264enc key-int-max=30 bitrate=100 \
        ! mpegtsmux \
        ! pravegasink stream=examples/stream1",
    )
    .unwrap();

    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.get_bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
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
