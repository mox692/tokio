use io_uring::{squeue::Entry, IoUring};
use mio::unix::SourceFd;
use slab::Slab;

use crate::runtime::driver::op::Lifecycle;
use crate::{io::Interest, loom::sync::Mutex};

use super::{Handle, TOKEN_URING};

use std::os::fd::AsRawFd;
use std::{io, mem, ops::DerefMut, task::Waker};

const DEFAULT_RING_SIZE: u32 = 8192;

pub(crate) struct UringHandle {
    pub(crate) uring: io_uring::IoUring,
    pub(crate) ops: slab::Slab<Lifecycle>,
}

impl UringHandle {
    pub(crate) fn new() -> Self {
        Self {
            ops: Slab::new(),
            // TODO: make configurable
            uring: IoUring::new(DEFAULT_RING_SIZE).unwrap(),
        }
    }
}

impl Handle {
    /// Called when runtime starts.
    #[allow(dead_code)]
    pub(crate) fn add_uring_source(&self, interest: Interest) -> io::Result<()> {
        // setup for io_uring
        let uringfd = self.get_uring().lock().uring.as_raw_fd();
        let mut source = SourceFd(&uringfd);
        self.registry
            .register(&mut source, TOKEN_URING, interest.to_mio())
    }

    pub(crate) fn get_uring(&self) -> &Mutex<UringHandle> {
        &self.uring_handle
    }

    pub(crate) fn register_op(&self, entry: Entry, waker: Waker) -> io::Result<usize> {
        let mut guard = self.get_uring().lock();
        let lock = guard.deref_mut();
        let ring = &mut lock.uring;
        let ops = &mut lock.ops;
        let index = ops.insert(Lifecycle::Waiting(waker));
        let entry = entry.user_data(index as u64);

        while unsafe { ring.submission().push(&entry).is_err() } {
            // If the submission queue is full, flush it to the kernel
            ring.submit().unwrap();
        }

        ring.submit()?;

        drop(guard);

        Ok(index)
    }

    pub(crate) fn deregister_op(&self, index: usize) {
        let mut guard = self.get_uring().lock();
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
}
