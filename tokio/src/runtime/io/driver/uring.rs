use io_uring::{squeue::Entry, IoUring};
use nix::sys::eventfd::{EfdFlags, EventFd};
use slab::Slab;

use crate::{io::Interest, loom::sync::Mutex, runtime::context::Lifecycle};

use super::{Driver, Handle};

use std::{
    io, mem,
    ops::DerefMut,
    os::fd::{AsFd, AsRawFd},
    task::Waker,
};

pub(crate) struct UringContext {
    pub(crate) uring: io_uring::IoUring,
    pub(crate) ops: slab::Slab<Lifecycle>,
    // TODO: we could eliminate this eventfd, like tokio-uring does?
    pub(crate) eventfd: EventFd,
}

impl UringContext {
    pub(crate) fn new() -> Self {
        // TODO: we could eliminate this eventfd, like tokio-uring does? In that case,
        //       I guess we should just pass the fd of the uring to the epoll_ctl.
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

    pub(crate) fn get_uring(&self, worker_index: usize) -> &Mutex<UringContext> {
        &self.uring_contexts[worker_index]
    }

    pub(crate) fn register_op(&self, worker_id: u64, entry: Entry, waker: Waker) -> usize {
        let mut guard = self.get_uring(worker_id as usize).lock();
        let lock = guard.deref_mut();
        let ring = &mut lock.uring;
        let ops = &mut lock.ops;
        let index = ops.insert(Lifecycle::Waiting(waker));

        unsafe {
            ring.submission()
                .push(&entry)
                .expect("submission queue is full");
        }

        let _ = ring.submit().expect("submit failed");

        drop(guard);

        index
    }

    pub(crate) fn deregister_op(&self, worker_id: u64, index: usize) {
        let mut guard = self.get_uring(worker_id as usize).lock();
        let lock = guard.deref_mut();
        let ops = &mut lock.ops;
        let Some(lifecycle) = ops.get_mut(index) else {
            // this Op is already done.
            return;
        };

        // this Op will be cancelled.

        match mem::replace(lifecycle, Lifecycle::Cancelled) {
            Lifecycle::Submitted | Lifecycle::Waiting(_) => (),
            // We should not see a Complete state here.
            _ => unreachable!(),
        };
    }
}

impl Driver {}
