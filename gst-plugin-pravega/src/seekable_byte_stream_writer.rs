//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use pravega_client::byte_stream::ByteStreamWriter;
use std::io::{Error, ErrorKind, Result, Seek, SeekFrom, Write};

/// A ByteStreamWriter that implements Seek.
pub struct SeekableByteStreamWriter {
    inner: ByteStreamWriter,
}

impl SeekableByteStreamWriter {
    pub fn new(writer: ByteStreamWriter) -> Result<SeekableByteStreamWriter> {
        let writer = SeekableByteStreamWriter {
            inner: writer
        };
        Ok(writer)
    }

    /// Gets a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut ByteStreamWriter {
        &mut self.inner
    }
}

impl Write for SeekableByteStreamWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl Seek  for SeekableByteStreamWriter {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.inner.current_write_offset() as u64),
            _ => Err(Error::new(ErrorKind::InvalidInput, "Seek is not allowed")),
        }
    }
}
