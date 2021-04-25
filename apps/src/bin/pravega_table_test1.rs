//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow::anyhow;
use clap::Clap;
use log::info;

use serde::{Deserialize, Serialize};
// use serde_json::Result;
use std::convert::TryInto;
use std::io::{Error, ErrorKind, Read, Write};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use pravega_client::client_factory::ClientFactory;
use pravega_client::tablemap::{TableError, TableMap, Version};
use pravega_client_config::ClientConfigBuilder;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment, StreamConfiguration, ScopedStream, Scaling, ScaleType};

#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
    /// Pravega scope
    #[clap(long, default_value = "examples")]
    scope: String,
    /// Pravega stream
    #[clap(long, default_value = "table1")]
    table: String,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
struct MyValue {
    pts: u64,
}

/// Demonstrate tablemap.
fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();
    let client_config = ClientConfigBuilder::default()
        .controller_uri(opts.controller)
        .build()
        .expect("creating config");
    let client_factory = ClientFactory::new(client_config);
    let runtime = client_factory.get_runtime();

    let scope = Scope::from(opts.scope);
    let table_name = format!("{}-{}", opts.table, Uuid::new_v4());
    info!("table_name={}", table_name);

    runtime.block_on(async {
        let map = client_factory.create_table_map(scope, table_name).await;

        // let k: String = "key".into();
        // let v: String = "val".into();
        // let r = map.insert(&k, &v, -1).await;
        // info!("==> PUT {:?}", r);
        // let r: Result<Option<(String, Version)>, TableError> = map.get(&k).await;
        // info!("==> GET {:?}", r);

        let k: String = "key2".into();
        let v = MyValue { pts: 123 };
        let r = map.insert(&k, &v, -1).await;
        info!("==> PUT {:?}", r);
        let r: Result<Option<(MyValue, Version)>, TableError> = map.get(&k).await;
        info!("==> GET {:?}", r);
    });

    println!("Done.");
}
