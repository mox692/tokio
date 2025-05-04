use io_uring::{squeue::Entry, IoUring};
use mio::unix::SourceFd;
use slab::Slab;

use crate::runtime::driver::op::{Lifecycle, Op};
use crate::{io::Interest, loom::sync::Mutex};

use super::{Handle, TOKEN_URING};

use std::os::fd::AsRawFd;
use std::{io, mem, task::Waker};

const DEFAULT_RING_SIZE: u32 = 256;

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

/// Drop the driver, cancelling any in-progress ops and waiting for them to terminate.
impl Drop for UringContext {
    fn drop(&mut self) {
        // Make sure we flush the submission queue before dropping the driver.
        while !self.uring.submission().is_empty() {
            submit(&mut self.uring, &mut self.ops).expect("Internal error when dropping driver");
        }

        let mut cancel_ops = Slab::new();
        let mut keys_to_move = Vec::new();

        for (key, lifecycle) in self.ops.iter() {
            match lifecycle {
                Lifecycle::Waiting(_) | Lifecycle::Submitted | Lifecycle::Cancelled(_) => {
                    // these should be cancelled
                    keys_to_move.push(key);
                }
                // We don't wait for completed ops.
                Lifecycle::Completed(_) => {}
            }
        }

        for key in keys_to_move {
            let lifecycle = self.ops.remove(key);
            cancel_ops.insert(lifecycle);
        }

        while !cancel_ops.is_empty() {
            // Wait until at least one completion is available.
            self.uring
                .submit_and_wait(1)
                .expect("Internal error when dropping driver");

            dispatch_completions(&mut self.uring, &mut cancel_ops);
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
            if let Err(e) = submit(ring, ops) {
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

fn submit(ring: &mut IoUring, ops: &mut Slab<Lifecycle>) -> io::Result<()> {
    // Handle errors: https://man7.org/linux/man-pages/man2/io_uring_enter.2.html#ERRORS
    loop {
        match ring.submit() {
            Ok(_) => {
                return Ok(());
            }
            // If the submission queue is full, we dispatch completions and try again.
            Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => {
                dispatch_completions(ring, ops);
            }
            // For other errors, we currently return the error as is.
            Err(e) => {
                return Err(e);
            }
        }
    }
}

pub(crate) fn dispatch_completions(uring: &mut IoUring, ops: &mut Slab<Lifecycle>) {
    let cq = uring.completion();

    for cqe in cq {
        let idx = cqe.user_data() as usize;

        match ops.get_mut(idx) {
            Some(Lifecycle::Waiting(waker)) => {
                waker.wake_by_ref();
                *ops.get_mut(idx).unwrap() = Lifecycle::Completed(cqe);
            }
            Some(Lifecycle::Cancelled(_)) => {
                // Op future was cancelled, so we discard the result.
                // We just remove the entry from the slab.
                ops.remove(idx);
            }
            Some(other) => {
                panic!("unexpected lifecycle for slot {}: {:?}", idx, other);
            }
            None => {
                panic!("no op at index {}", idx);
            }
        }
    }

    // `cq`'s drop gets called here, updating the latest head pointer
}
