/*
 *  Copyright (c) 2017-present, Facebook, Inc.
 *  All rights reserved.
 *
 *  This source code is licensed under the BSD-style license found in the
 *  LICENSE file in the root directory of this source tree. An additional grant
 *  of patent rights can be found in the PATENTS file in the same directory.
 *
 */
//! This module contains a writer wrapper that breaks writes up according to a
//! provided iterator.

use std::cmp;
use std::fmt;
use std::io::{self, Write};

use {PartialOp, make_ops};

/// A writer wrapper that breaks inner `Write` instances up according to the
/// provided iterator.
///
/// # Examples
///
/// ```rust
/// use std::io::Write;
///
/// use partial_io::{PartialOp, PartialWrite};
///
/// let writer = Vec::new();
/// let iter = ::std::iter::repeat(PartialOp::Limited(1));
/// let mut partial_writer = PartialWrite::new(writer, iter);
/// let in_data = vec![1, 2, 3, 4];
///
/// let size = partial_writer.write(&in_data).unwrap();
/// assert_eq!(size, 1);
/// assert_eq!(&partial_writer.get_ref()[..], &[1]);
/// ```
pub struct PartialWrite<W> {
    inner: W,
    ops: Box<Iterator<Item = PartialOp>>,
}

impl<W> PartialWrite<W>
    where W: Write
{
    /// Creates a new `PartialWrite` wrapper over the writer with the specified `PartialOp`s.
    pub fn new<I>(inner: W, iter: I) -> Self
        where I: IntoIterator<Item = PartialOp> + 'static
    {
        PartialWrite {
            inner: inner,
            // Use fuse here so that we don't keep calling the inner iterator
            // once it's returned None.
            ops: make_ops(iter),
        }
    }

    /// Sets the `PartialOp`s for this writer.
    pub fn set_ops<I>(&mut self, iter: I) -> &mut Self
        where I: IntoIterator<Item = PartialOp> + 'static
    {
        self.ops = make_ops(iter);
        self
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Consumes this wrapper, returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W> Write for PartialWrite<W>
    where W: Write
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.ops.next() {
            Some(PartialOp::Limited(n)) => {
                let len = cmp::min(n, buf.len());
                self.inner.write(&buf[..len])
            }
            Some(PartialOp::Err(err)) => {
                Err(io::Error::new(err, "error during write, generated by partial-io"))
            }
            Some(PartialOp::Unlimited) |
            None => self.inner.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.ops.next() {
            Some(PartialOp::Err(err)) => {
                Err(io::Error::new(err, "error during flush, generated by partial-io"))
            }
            _ => self.inner.flush(),
        }
    }
}

impl<W> fmt::Debug for PartialWrite<W>
    where W: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PartialWrite")
            .field("inner", &self.inner)
            .finish()
    }
}
