//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use std::io::{Read, Result, Seek, SeekFrom, Take};

/// Reader adaptor which returns EOF beyond the specified end position.
#[derive(Debug)]
pub struct SeekableTake<T> {
    inner: Take<T>,
    end_position: u64,
}

impl<T: Read + Seek> SeekableTake<T> {
    pub fn new(reader: T, end_position: u64) -> Result<SeekableTake<T>> {
        let mut reader = SeekableTake {
            inner: reader.take(u64::MAX),
            end_position,
        };
        reader.seek(SeekFrom::Current(0))?;
        Ok(reader)
    }

    /// Consumes the `SeekableTake`, returning the wrapped reader.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &T {
        &self.inner.get_ref()
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `SeekableTake`.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T: Read> Read for SeekableTake<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.inner.read_to_end(buf)
    }
}

impl<T: Seek> Seek  for SeekableTake<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = self.inner.get_mut().seek(pos)?;
        if new_pos <= self.end_position {
            self.inner.set_limit(self.end_position - new_pos);
            Ok(new_pos)
        } else {
            self.inner.set_limit(0);
            self.inner.get_mut().seek(SeekFrom::Start(self.end_position))
        }
    }
}
