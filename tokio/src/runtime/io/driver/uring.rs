use io_uring::{squeue::Entry, IoUring};
use nix::sys::eventfd::{EfdFlags, EventFd};
use slab::Slab;

use crate::{
    io::Interest,
    loom::sync::Mutex,
    runtime::context::{Lifecycle, Op},
};

use super::{Driver, Handle};

use std::{
    io,
    os::fd::{AsFd, AsRawFd},
};

pub(crate) struct UringContext {
    pub(crate) uring: io_uring::IoUring,
    pub(crate) ops: slab::Slab<Lifecycle>,
    pub(crate) eventfd: EventFd,
}

impl UringContext {
    pub(crate) fn new() -> Self {
        let eventfd = EventFd::from_value_and_flags(0, EfdFlags::EFD_NONBLOCK).unwrap();
        let uring = IoUring::new(8).unwrap();
        uring
            .submitter()
            .register_eventfd(eventfd.as_fd().as_raw_fd())
            .unwrap();

        Self {
            ops: Slab::new(),
            uring,
            eventfd,
        }
    }
}

pub(super) fn is_uring_token(token: mio::Token) -> bool {
    token.0 & (1 << 63) != 0
}

pub(super) fn get_worker_index(token: mio::Token) -> usize {
    (token.0 & 0x7FFF_FFFF_FFFF_FFFF) as usize
}

fn uring_token(index: usize) -> mio::Token {
    mio::Token(index | (1 << 63))
}

impl Handle {
    /// Called when runtime starts.
    pub(crate) fn add_uring_source(
        &self,
        source: &mut impl mio::event::Source,
        worker_index: usize,
        interest: Interest,
    ) -> io::Result<()> {
        self.registry
            .register(source, uring_token(worker_index), interest.to_mio())
    }

    pub(crate) fn get_uring(&self, index: usize) -> &Mutex<UringContext> {
        &self.uring_contexts[index]
    }

    pub(crate) fn register_op<T>(&self, index: usize, sqe: Entry, data: T) -> Op<T> {
        let mut ctx = self.uring_contexts[index].lock();
        let index = ctx
            .ops
            .insert(crate::runtime::context::Lifecycle::Submitted);

        let ring = &mut ctx.uring;

        // Safety: We're assuming `open_op` is valid and holding a lock for this ring.
        unsafe {
            ring.submission()
                .push(&sqe.user_data(index as u64))
                .expect("submission queue is full");
        }

        // Submit without waiting.
        // TODO: Consider batching in the future.
        let _ = ring.submit().expect("submit failed");

        Op::new(index, data)
    }
}

impl Driver {}
