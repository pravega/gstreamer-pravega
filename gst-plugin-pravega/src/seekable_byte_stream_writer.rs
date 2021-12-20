//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#![allow(dead_code)]

use pravega_client::byte::ByteWriter;
use std::io::{Error, ErrorKind, Result, Seek, SeekFrom, Write};
use tokio::runtime::Handle;

/// A ByteWriter that implements Seek.
pub struct SeekableByteWriter {
    inner: ByteWriter,
    runtime_handle: Handle,
}

impl SeekableByteWriter {
    pub fn new(writer: ByteWriter, runtime_handle: Handle) ->  Self {
        Self {
            inner: writer,
            runtime_handle,
        }
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &ByteWriter {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut ByteWriter {
        &mut self.inner
    }

    pub fn seal(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.seal()).map_err(|err|{Error::new(ErrorKind::Other, err.to_string())})
    }

    pub fn seek_to_tail(&mut self) {
        self.runtime_handle.block_on(self.inner.seek_to_tail())
    }
}

impl Write for SeekableByteWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.runtime_handle.block_on(self.inner.write(buf)).map_err(|err|{Error::new(ErrorKind::Other, err.to_string())})
    }

    fn flush(&mut self) -> Result<()> {
        self.runtime_handle.block_on(self.inner.flush()).map_err(|err|{Error::new(ErrorKind::Other, err.to_string())})
    }
}

impl Seek for SeekableByteWriter {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.inner.current_offset() as u64),
            _ => Err(Error::new(ErrorKind::InvalidInput, "Seek is not allowed")),
        }
    }
}
