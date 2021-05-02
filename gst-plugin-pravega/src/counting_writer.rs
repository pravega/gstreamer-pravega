//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use std::io::{Write, Result, Seek, SeekFrom};

/// Write adaptor that tracks the current offset so it can be returned without seeking the inner writer.
#[derive(Debug)]
pub struct CountingWriter<T: Write + Seek> {
    inner: T,
    offset: u64,
}

impl<T: Write + Seek> CountingWriter<T> {
    pub fn new(mut writer: T) -> Result<CountingWriter<T>> {
        let offset = writer.seek(SeekFrom::Current(0))?;
        let writer = CountingWriter {
            inner: writer,
            offset
        };
        Ok(writer)
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying writer as doing so may corrupt the offset.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Write + Seek> Write for CountingWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let result = self.inner.write(buf);
        if let Ok(written) = result {
            self.offset += written as u64;
        }
        result
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl<T: Write + Seek> Seek  for CountingWriter<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.offset),
            _ => {
                self.offset = self.inner.seek(pos)?;
                Ok(self.offset)
            }
        }
    }
}
