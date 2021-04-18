use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    // This creates a pipeline by parsing the gst-launch pipeline syntax.
    let pipeline = gst::parse_launch(
        "playbin uri=file:///home/faheyc/nautilus/gstreamer/gstreamer-pravega/test.ts",
    )
    .unwrap();

    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

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
