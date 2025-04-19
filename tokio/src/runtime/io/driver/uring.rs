use io_uring::{squeue::Entry, IoUring};
use mio::unix::SourceFd;
use slab::Slab;

use crate::runtime::driver::op::Lifecycle;
use crate::{io::Interest, loom::sync::Mutex};

use super::Handle;

use std::os::fd::AsRawFd;
use std::{io, mem, ops::DerefMut, task::Waker};

const DEFAULT_RING_SIZE: u32 = 8192;

pub(crate) struct UringContext {
    pub(crate) uring: io_uring::IoUring,
    pub(crate) ops: slab::Slab<Lifecycle>,
}

impl UringContext {
    pub(crate) fn new() -> Self {
        Self {
            ops: Slab::new(),
            // TODO: make configurable
            uring: IoUring::new(DEFAULT_RING_SIZE).unwrap(),
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
        worker_index: usize,
        interest: Interest,
    ) -> io::Result<()> {
        // setup for io_uring
        let uringfd = self.get_uring(worker_index).lock().uring.as_raw_fd();
        let mut source = SourceFd(&uringfd);
        self.registry
            .register(&mut source, uring_token(worker_index), interest.to_mio())
    }

    pub(crate) fn get_uring(&self, worker_index: usize) -> &Mutex<UringContext> {
        &self.uring_contexts
    }

    pub(crate) fn register_op(&self, worker_id: u64, entry: Entry, waker: Waker) -> usize {
        let mut guard = self.get_uring(worker_id as usize).lock();
        let lock = guard.deref_mut();
        let ring = &mut lock.uring;
        let ops = &mut lock.ops;
        let index = ops.insert(Lifecycle::Waiting(waker));
        let entry = entry.user_data(index as u64);

        while unsafe { ring.submission().push(&entry).is_err() } {
            // If the submission queue is full, flush it to the kernel
            ring.submit().unwrap();
        }

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
            prev => panic!("Unexpected state: {:?}", prev),
        };
    }

    pub(crate) fn uring_len(&self) -> usize {
        1
        // self.uring_contexts.len()
    }
}
