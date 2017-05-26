/*
 *  Copyright (c) 2017-present, Facebook, Inc.
 *  All rights reserved.
 *
 *  This source code is licensed under the BSD-style license found in the
 *  LICENSE file in the root directory of this source tree. An additional grant
 *  of patent rights can be found in the PATENTS file in the same directory.
 *
 */
//! This module contains an `AsyncWrite` wrapper that breaks writes up
//! according to a provided iterator.
//!
//! This is separate from `PartialWrite` because on `WouldBlock` errors, it
//! causes `futures` to try writing or flushing again.

use std::cmp;
use std::io::{self, Write};
use std::iter::Fuse;

use futures::{Poll, task};
use tokio_io::AsyncWrite;

use PartialOp;

/// A wrapper that breaks inner `AsyncWrite` instances up according to the
/// provided iterator.
///
/// Available with the `tokio` feature.
///
/// # Examples
///
/// ```rust
/// extern crate partial_io;
/// extern crate tokio_core;
/// extern crate tokio_io;
///
/// use std::io::{self, Cursor};
///
/// fn main() {
///     // Note that this test doesn't demonstrate a limited write because
///     // tokio-io doesn't have a combinator for that, just write_all.
///     use tokio_core::reactor::Core;
///     use tokio_io::io::write_all;
///
///     use partial_io::{PartialAsyncWrite, PartialOp};
///
///     let writer = Cursor::new(Vec::new());
///     let iter = vec![PartialOp::Err(io::ErrorKind::WouldBlock), PartialOp::Limited(2)];
///     let partial_writer = PartialAsyncWrite::new(writer, iter);
///     let in_data = vec![1, 2, 3, 4];
///
///     let mut core = Core::new().unwrap();
///
///     let write_fut = write_all(partial_writer, in_data);
///
///     let (partial_writer, _in_data) = core.run(write_fut).unwrap();
///     let cursor = partial_writer.into_inner();
///     let out = cursor.into_inner();
///     assert_eq!(&out, &[1, 2, 3, 4]);
/// }
/// ```
pub struct PartialAsyncWrite<W, I>
    where I: IntoIterator<Item = PartialOp>
{
    inner: W,
    iter: Fuse<I::IntoIter>,
}

impl<W, I> PartialAsyncWrite<W, I>
    where W: AsyncWrite,
          I: IntoIterator<Item = PartialOp>
{
    pub fn new(inner: W, iter: I) -> Self {
        PartialAsyncWrite {
            inner: inner,
            // Use fuse here so that we don't keep calling the inner iterator
            // once it's returned None.
            iter: iter.into_iter().fuse(),
        }
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

impl<W, I> Write for PartialAsyncWrite<W, I>
    where W: Write,
          I: IntoIterator<Item = PartialOp>
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.iter.next() {
            Some(PartialOp::Limited(n)) => {
                let len = cmp::min(n, buf.len());
                self.inner.write(&buf[..len])
            }
            Some(PartialOp::Err(err)) => {
                if err == io::ErrorKind::WouldBlock {
                    // Make sure this task is rechecked.
                    task::park().unpark();
                }
                Err(io::Error::new(err, "error during write, generated by partial-io"))
            }
            Some(PartialOp::Unlimited) |
            None => self.inner.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.iter.next() {
            Some(PartialOp::Err(err)) => {
                Err(io::Error::new(err, "error during flush, generated by partial-io"))
            }
            _ => self.inner.flush(),
        }
    }
}

impl<W, I> AsyncWrite for PartialAsyncWrite<W, I>
    where W: AsyncWrite,
          I: IntoIterator<Item = PartialOp>
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.inner.shutdown()
    }
}