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
use pravega_client::client_factory::ClientFactory;
use pravega_client_config::ClientConfigBuilder;
use tracing_subscriber::fmt::format::FmtSpan;
use warp::Filter;

/// Serve HTTP Live Streaming (HLS) from a Pravega MPEG Transport Stream.
/// Point your browser to: http://localhost:3030/player?scope=examples&stream=hlsav4
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "pravega_video_server=debug,warp=debug,debug".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();
    tracing::info!("main: BEGIN");

    // Let Pravega ClientFactory create the Tokio runtime. It will also be used by Warp.

    let controller_uri = opts.controller;
    let config = ClientConfigBuilder::default()
        .controller_uri(controller_uri)
        .build()
        .expect("creating config");
    let client_factory = ClientFactory::new(config);
    let client_factory_db = client_factory.clone();
    let runtime = client_factory.get_runtime();

    runtime.block_on(async {
        let db = models::new(client_factory_db);
        let api = filters::get_all_filters(db);
        let ui = ui::get_all_filters();
        let static_dir = warp::path("static").and(warp::fs::dir("./static"));
        // let redirect = warp::path::end().map(|| {
        //     warp::redirect::temporary(Uri::from_static("/static/hls-js.html"))
        // });
        // TODO: For testing, configure CORS to allow access from any origin. This needs to be removed or limited for production.
        let cors = warp::cors().allow_any_origin();
        let routes = api
            .or(ui)
            .or(static_dir)
            // .or(redirect)
            .with(cors)
            .with(warp::trace::request());
        warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
    })
}
mod filters {
    use super::handlers;
    use super::models::{Db, GetMpegTransportStreamOptions, GetM3u8PlaylistOptions};
    use warp::Filter;

    pub fn get_all_filters(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        get_mpeg_transport_stream(db.clone())
            .or(get_m3u8_playlist(db.clone()))
            .or(list_video_streams(db.clone()))
    }

    /// GET /scopes/my_scope/streams/my_stream/ts?begin=0&end=204
    pub fn get_mpeg_transport_stream(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scopes" / String / "streams" / String / "ts" )
            .and(warp::get())
            .and(warp::query::<GetMpegTransportStreamOptions>())
            .and(with_db(db))
            .and_then(handlers::get_mpeg_transport_stream)
    }

    /// GET /scopes/my_scope/streams/my_stream/m3u8?begin=2021-04-19T00:00:00Z&end=2021-04-20T00:00:00Z
    pub fn get_m3u8_playlist(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scopes" / String / "streams" / String / "m3u8" )
            .and(warp::get())
            .and(warp::query::<GetM3u8PlaylistOptions>())
            .and(with_db(db))
            .and_then(handlers::get_m3u8_playlist)
            .with(warp::compression::gzip())
    }

    /// List streams within the given scope
    /// GET /scopes/my_scope/streams
    pub fn list_video_streams(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scopes" / String / "streams")
            .and(warp::get())
            .and(with_db(db))
            .and_then(handlers::list_video_streams)
    }

    fn with_db(db: Db) -> impl Filter<Extract = (Db,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || db.clone())
    }
}

mod ui {
    use chrono::{DateTime, Utc};
    use handlebars::Handlebars;
    use serde_derive::{Deserialize, Serialize};
    use warp::Filter;

    #[derive(Debug, Deserialize, Serialize)]
    pub struct GetPlayerHtmlOptions {
        #[serde(rename = "scope")]
        pub scope_name: String,
        #[serde(rename = "stream")]
        pub stream_name: String,
        pub begin: Option<DateTime<Utc>>,
        pub end: Option<DateTime<Utc>>,
    }

    pub fn get_all_filters(
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        get_player_html()
    }

    pub fn get_player_html(
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("player")
            .and(warp::get())
            .and(warp::query::<GetPlayerHtmlOptions>())
            .map(|opts: GetPlayerHtmlOptions| {
                let mut hb = Handlebars::new();
                let template_name = "player.html";
                hb.register_template_file(template_name, "templates/player.html").unwrap();
                let html = hb.render(template_name, &opts).unwrap();
                Ok(warp::reply::html(html))
                })
    }
}

mod handlers {
    use std::convert::Infallible;
    use super::models::{Db, GetMpegTransportStreamOptions, GetM3u8PlaylistOptions};

    pub async fn get_mpeg_transport_stream(
        scope_name: String,
        stream_name: String,
        opts: GetMpegTransportStreamOptions,
        db: Db,
    ) -> Result<impl warp::Reply, Infallible> {
        db.get_mpeg_transport_stream(scope_name, stream_name, opts).await
    }

    pub async fn get_m3u8_playlist(
        scope_name: String,
        stream_name: String,
        opts: GetM3u8PlaylistOptions,
        db: Db,
    ) -> Result<impl warp::Reply, Infallible> {
        let playlist = db.get_m3u8_playlist(scope_name, stream_name, opts).await.unwrap();
        Ok(warp::reply::with_header(playlist, "content-type", "application/x-mpegURL"))
    }

    pub async fn list_video_streams(
        scope_name: String,
        db: Db,
    ) -> Result<impl warp::Reply, Infallible> {
        tracing::info!("list_video_streams: scope_name={}", scope_name);
        let streams = db.list_video_streams(scope_name).await.unwrap();
        Ok(warp::reply::json(&streams))
    }
}

mod models {
    use anyhow;
    use chrono::{DateTime, Utc};
    use futures_util::StreamExt;
    use hyper::body::{Body, Bytes};
    use pravega_client::client_factory::ClientFactory;
    use pravega_client_shared::{Scope, ScopedSegment, Segment, Stream};
    use pravega_video::{event_serde::{EventReader}, index::IndexSearcher};
    use pravega_video::index::{IndexRecord, IndexRecordReader, SearchMethod, get_index_stream_name};
    use pravega_video::timestamp::PravegaTimestamp;
    use serde_derive::{Deserialize, Serialize};
    use std::convert::Infallible;
    use std::io::{ErrorKind, Read, Seek, SeekFrom};

    #[derive(Clone)]
    pub struct Db {
        pub client_factory: ClientFactory,
    }

    pub fn new(client_factory: ClientFactory) -> Db {
        Db { client_factory }
    }

    // The query parameters for get_mpeg_transport_stream.
    #[derive(Debug, Deserialize)]
    pub struct GetMpegTransportStreamOptions {
        /// Begin byte offset
        pub begin: u64,
        /// End byte offset (exclusive)
        pub end: u64,
    }

    // The query parameters for get_m3u8_playlist.
    #[derive(Debug, Deserialize)]
    pub struct GetM3u8PlaylistOptions {
        pub begin: Option<DateTime<Utc>>,
        pub end: Option<DateTime<Utc>>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct ListStreamsResult {
        pub streams: Vec<ListStreamsRecord>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct ListStreamsRecord {
        #[serde(rename = "scopeName")]
        pub scope_name: String,
        #[serde(rename = "streamName")]
        pub stream_name: String,
    }

    impl Db {
        pub async fn get_mpeg_transport_stream(
            self,
            scope_name: String,
            stream_name: String,
            opts: GetMpegTransportStreamOptions,
        ) -> Result<impl warp::Reply, Infallible> {
            tracing::info!("scope_name={}, stream_name={}, begin={}, end={}", scope_name, stream_name, opts.begin, opts.end);
            assert!(opts.begin <= opts.end);

            // TODO: Provide chunks to the HTTP client as a stream instead of buffering the entire response.

            // Use spawn_blocking to allow Pravega non-async methods to block this thread.
            // See https://stackoverflow.com/a/65452213/5890553.

            let chunks = tokio::task::spawn_blocking(move || {
                let client_factory = self.client_factory;
                let scoped_segment = ScopedSegment {
                    scope: Scope::from(scope_name),
                    stream: Stream::from(stream_name),
                    segment: Segment::from(0),
                };
                let mut reader = client_factory.create_byte_stream_reader(scoped_segment);
                tracing::info!("Opened Pravega reader");

                reader.seek(SeekFrom::Start(opts.begin)).unwrap();
                let limit = opts.end - opts.begin;
                let mut reader = reader.take(limit);

                let mut chunks: Vec<Result<Bytes, std::io::Error>> = Vec::new();

                loop {
                    let mut event_reader = EventReader::new();
                    let required_buffer_length =
                        match event_reader.read_required_buffer_length(&mut reader) {
                            Ok(n) => n,
                            Err(e) if e.kind() == ErrorKind::UnexpectedEof && reader.limit() == 0 => {
                                tracing::trace!("Reached requested end");
                                break;
                            },
                            Err(e) => return Err(e),
                    };
                    let mut read_buffer: Vec<u8> = vec![0; required_buffer_length];
                    let event = match event_reader.read_event(&mut reader, &mut read_buffer[..]) {
                        Ok(n) => n,
                        Err(e) if e.kind() == ErrorKind::UnexpectedEof && reader.limit() == 0 => {
                            tracing::trace!("Reached requested end");
                            break;
                        },
                        Err(e) => return Err(e),
                    };
                    tracing::trace!("get_mpeg_transport_stream: event={:?}", event);
                    chunks.push(Ok(Bytes::copy_from_slice(&event.payload)));
                }
                tracing::info!("Created {} chunks", chunks.len());
                assert!(reader.limit() == 0);
                Ok(chunks)
            })
            .await
            .unwrap()
            .unwrap();

            tracing::trace!("spawn_blocking done");
            let stream = futures_util::stream::iter(chunks);
            let body = Body::wrap_stream(stream);
            Ok(warp::reply::with_header(warp::reply::Response::new(body), "content-type", "video/MP2T"))
        }

        pub async fn get_m3u8_playlist(
            self,
            scope_name: String,
            stream_name: String,
            opts: GetM3u8PlaylistOptions,
        ) -> anyhow::Result<String> {
            tracing::info!("scope_name={}, stream_name={}, begin={:?}, end={:?}", scope_name, stream_name, opts.begin, opts.end);

            let index_stream_name = get_index_stream_name(&stream_name);
            let begin_timestamp = PravegaTimestamp::from(opts.begin).or(PravegaTimestamp::MIN);
            let end_timestamp = PravegaTimestamp::from(opts.end).or(PravegaTimestamp::MAX);
            tracing::info!("begin_timestamp={}, end_timestamp={}", begin_timestamp, end_timestamp);
            assert!(begin_timestamp <= end_timestamp);

            // Use spawn_blocking to allow Pravega non-async methods to block this thread.
            // See https://stackoverflow.com/a/65452213/5890553.

            let playlist = tokio::task::spawn_blocking(move || {
                let client_factory = self.client_factory;
                let scoped_segment = ScopedSegment {
                    scope: Scope::from(scope_name),
                    stream: Stream::from(index_stream_name),
                    segment: Segment::from(0),
                };
                let index_reader = client_factory.create_byte_stream_reader(scoped_segment);
                tracing::info!("Opened Pravega reader");

                let mut index_searcher = IndexSearcher::new(index_reader);
                let begin_index_record = index_searcher.search_timestamp_and_return_index_offset(
                    begin_timestamp, SearchMethod::After)?;
                let end_index_record = index_searcher.search_timestamp_and_return_index_offset(
                    end_timestamp, SearchMethod::After)?;
                // Determine whether we can possibly get more data in the future.
                // If the caller specified an end time and we already have an index record beyond this, then
                // future appends will not affect our result.
                // TODO: We can also guarantee this if the stream has been sealed.
                let have_all_data = end_index_record.0.timestamp >= end_timestamp;
                tracing::info!("begin_index_record={:?}, end_index_record={:?}, have_all_data={}",
                        begin_index_record, end_index_record, have_all_data);
                let mut index_reader = index_searcher.into_inner();

                // Determine begin and end offsets of the index.
                let index_begin_offset = begin_index_record.1;
                let index_end_offset = end_index_record.1 + IndexRecord::RECORD_SIZE as u64;
                let index_size = index_end_offset - index_begin_offset;
                tracing::info!("index_begin_offset={}, index_end_offset={}, index_size={}", index_begin_offset, index_end_offset, index_size);

                // Position index reader at current beginning of the index.
                index_reader.seek(SeekFrom::Start(index_begin_offset)).unwrap();

                // Ensure EOF instead of waiting (potentially forever) for appends when we get to the current end.
                let mut index_reader = index_reader.take(index_size);

                // Media Sequence Number will always equal the index record number, even after truncation.
                let initial_media_sequence_number: u64 = index_begin_offset / IndexRecord::RECORD_SIZE as u64;
                tracing::info!("initial_media_sequence_number={}", initial_media_sequence_number);

                // Initial value for target duration. This will be updated with an exponential moving average, then rounded.
                let mut target_duration_seconds = 10.0;

                let mut playlist_body = String::new();
                let mut prev_index_record: Option<IndexRecord> = None;
                let mut next_segment_discont = false;

                loop {
                    let mut index_record_reader = IndexRecordReader::new();
                    let index_record = match index_record_reader.read(&mut index_reader) {
                        Ok(n) => n,
                        Err(e) if e.kind() == ErrorKind::UnexpectedEof && index_reader.limit() == 0 => {
                            tracing::trace!("Reached requested end");
                            break;
                        },
                        Err(e) => return Err(e),
                    };
                    tracing::trace!("index_record={:?}", index_record);
                    if let Some(prev_index_record) = prev_index_record {
                        // If index_record indicates a discontinuity, then assume there is a gap in the data
                        // between the previous record and this one.
                        // Any recorded content that falls in this gap may be corrupt so we will not display it.
                        // Instead, we'll play a short transport stream containing blue video and silent audio.
                        // The length of this replacement content will be fixed, regardless of the timestamps.
                        // The EXT-X-GAP tag should be used for this but it doesn't appear to be supported by hls.js.
                        // It is possible that the duration of the gap in the index is very short or even 0.
                        // However, we still need to count the gap so that the Media Sequence Numbers
                        // correspond to the index offset.

                        let mut discont = index_record.discontinuity;
                        if discont {
                            tracing::warn!("Detected discontinuity; discontinuity flag set in {:?}", index_record);
                        } else {
                            if let Some(timestamp_nanos) = index_record.timestamp.nanoseconds() {
                                let prev_timestamp_nanos = prev_index_record.timestamp.nanoseconds().unwrap();
                                if timestamp_nanos < prev_timestamp_nanos {
                                    let rewind_seconds = (prev_timestamp_nanos - timestamp_nanos) as f64 * 1e-9;
                                    tracing::warn!("Detected discontinuity; rewind of {:.3} seconds from {} to {}",
                                    rewind_seconds, prev_index_record.timestamp, index_record.timestamp);
                                    discont = true;
                                } else {
                                    let duration_seconds = (timestamp_nanos - prev_timestamp_nanos) as f64 * 1e-9;
                                    // If the timestamp increased by much more than the target duration,
                                    // then assume we have a discontinuity.
                                    if duration_seconds > target_duration_seconds + 1.0 {
                                        tracing::warn!("Detected discontinuity; {:.3} second gap from {} to {}, target_duration_seconds={:.3}",
                                            duration_seconds, prev_index_record.timestamp, index_record.timestamp, target_duration_seconds);
                                        discont = true;
                                    } else {
                                        if next_segment_discont {
                                            playlist_body.push_str("#EXT-X-DISCONTINUITY\n");
                                            next_segment_discont = false;
                                        }
                                        let ema_alpha = 0.1;
                                        target_duration_seconds = ema_alpha * duration_seconds + (1.0 - ema_alpha) * target_duration_seconds;
                                        let begin_offset = prev_index_record.offset;
                                        let end_offset = index_record.offset;
                                        // "#EXTINF:10," where 10 is the duration of the segment in seconds
                                        playlist_body.push_str(&format!("#EXTINF:{},\n", duration_seconds));
                                        // "#EXT-X-PROGRAM-DATE-TIME:2010-02-19T14:54:23.123456789Z"
                                        playlist_body.push_str(&format!("#EXT-X-PROGRAM-DATE-TIME:{}\n", prev_index_record.timestamp.to_iso_8601().unwrap()));
                                        // "ts?begin=0&end=204" where 0 and 204 are the begin and end byte offsets
                                        playlist_body.push_str(&format!("ts?begin={}&end={}\n", begin_offset, end_offset));
                                    }
                                }
                            } else {
                                tracing::warn!("Detected discontinuity; missing timestamp in index at offset {}",
                                    index_record.offset);
                                discont = true;
                            }
                        }
                        if discont {
                            // tracing::warn!("Detected discontinuity; index_record={:?}", index_record);
                            let gap_content_duration_seconds = 5;
                            playlist_body.push_str("#EXT-X-DISCONTINUITY\n");
                            playlist_body.push_str(&format!("#EXTINF:{},\n", gap_content_duration_seconds));
                            playlist_body.push_str(&format!("/static/gap-{}s.ts\n", gap_content_duration_seconds));
                            next_segment_discont = true;
                        }
                    }
                    prev_index_record = Some(index_record);
                }

                let mut playlist = String::new();
                let target_duration_seconds = target_duration_seconds.round();
                tracing::info!("target_duration_seconds={}", target_duration_seconds);
                playlist.push_str("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-ALLOW-CACHE:NO\n");
                playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", initial_media_sequence_number));
                playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", target_duration_seconds));
                playlist.push_str(&playlist_body);

                // Write ENDLIST if we have all data up to the requested end time.
                // This will prevent the browser from polling for updated playlists.
                if have_all_data {
                    playlist.push_str("#EXT-X-ENDLIST\n");
                }
                Ok(playlist)
            })
            .await??;
            tracing::trace!("spawn_blocking done");
            tracing::trace!("playlist={}", playlist);
            Ok(playlist)
        }

        pub async fn list_video_streams(
            self,
            scope_name: String,
        ) -> anyhow::Result<ListStreamsResult> {
            use futures::future;
            use pravega_controller_client::paginator::list_streams;
            use tokio::runtime::Runtime;
            let rt = Runtime::new().unwrap()
            ;
            let ss = scope_name.clone();

            tracing::info!("list_video_streams: scope_name={}", scope_name);
            let controller_client = self.client_factory.get_controller_client();
            let scope = Scope { name : scope_name };
            let mut streams = Vec::new();
            rt.block_on(list_streams(scope, controller_client).for_each(|stream| {
                if stream.is_ok() {
                    streams.push(stream.unwrap());
                } else {
                    println!("Error while fetching data from Controller. Details: {:?}", stream);
                }
                future::ready(())
            }));
            let streams: Vec<_> = streams.into_iter().map(|scoped_stream| ListStreamsRecord {
                scope_name: ss.clone(),
                stream_name: scoped_stream.stream.name
            }).collect();
            Ok(ListStreamsResult { streams })
        }
    }
}
