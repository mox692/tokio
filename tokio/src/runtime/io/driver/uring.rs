use io_uring::{squeue::Entry, IoUring};
use mio::unix::SourceFd;
use slab::Slab;

use crate::runtime::driver::op::{Lifecycle, Op};
use crate::{io::Interest, loom::sync::Mutex};

use super::{Handle, TOKEN_URING};

use std::os::fd::AsRawFd;
use std::{io, mem, task::Waker};

const DEFAULT_RING_SIZE: u32 = 512;

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

impl Handle {
    #[allow(dead_code)]
    pub(crate) fn add_uring_source(&self, interest: Interest) -> io::Result<()> {
        // setup for io_uring
        let uringfd = self.get_uring().lock().uring.as_raw_fd();
        let mut source = SourceFd(&uringfd);
        self.registry
            .register(&mut source, TOKEN_URING, interest.to_mio())
    }

    pub(crate) fn get_uring(&self) -> &Mutex<UringContext> {
        &self.uring_context
    }

    /// # Safety
    ///
    /// Callers must ensure that parameters of the entry (such as buffer) are valid and will
    /// be valid for the entire duration of the operation, otherwise it may cause memory problems.
    pub(crate) unsafe fn register_op(&self, entry: Entry, waker: Waker) -> io::Result<usize> {
        let mut guard = self.get_uring().lock();
        let ctx = &mut *guard;
        let ring = &mut ctx.uring;
        let ops = &mut ctx.ops;
        let index = ops.insert(Lifecycle::Waiting(waker));
        let entry = entry.user_data(index as u64);

        let mut submit_or_remove = |ring: &mut IoUring| -> io::Result<()> {
            if let Err(e) = submit(ring) {
                // Submission failed, remove the entry from the slab and return the error
                ops.remove(index);
                return Err(e);
            }
            Ok(())
        };

        // SAFETY: entry is valid for the entire duration of the operation
        while unsafe { ring.submission().push(&entry).is_err() } {
            // If the submission queue is full, flush it to the kernel
            submit_or_remove(ring)?;
        }

        // For now, we submit the entry immediately without utilizing batching.
        submit_or_remove(ring)?;

        Ok(index)
    }

    pub(crate) fn cancel_op<T: Send + 'static>(&self, op: &mut Op<T>, index: usize) {
        let mut guard = self.get_uring().lock();
        let ctx = &mut *guard;
        let ops = &mut ctx.ops;
        let Some(lifecycle) = ops.get_mut(index) else {
            // The corresponding index doesn't exist anymore, so this Op is already complete.
            return;
        };

        // This Op will be cancelled. Here, we don't remove the lifecycle from the slab to keep
        // uring data alive until the operation completes.

        match mem::replace(lifecycle, Lifecycle::Cancelled(Box::new(op.take_data()))) {
            Lifecycle::Submitted | Lifecycle::Waiting(_) => (),
            prev => panic!("Unexpected state: {:?}", prev),
        };
    }
}

fn submit(ring: &IoUring) -> io::Result<()> {
    // Handle errors: https://man7.org/linux/man-pages/man2/io_uring_enter.2.html#ERRORS
    match ring.submit() {
        Ok(_) => {
            return Ok(());
        }
        Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => {
            // TODO: trigger a completion flush
        }
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}
