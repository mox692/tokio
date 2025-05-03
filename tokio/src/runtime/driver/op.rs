use io_uring::cqueue;
use io_uring::squeue::Entry;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::{io, mem};

#[derive(Debug)]
pub(crate) enum Lifecycle {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,

    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),

    /// The submitter no longer has interest in the operation result. The state
    /// must be passed to the driver and held until the operation completes.
    Cancelled,

    /// The operation has completed with a single cqe result
    Completed(io_uring::cqueue::Entry),
}

pub(crate) enum State {
    #[allow(dead_code)]
    Initialize(Option<Entry>),
    Polled(usize), // slab key
    Complete,
}

pub(crate) struct Op<T> {
    // State of this Op
    state: State,
    // Per operation data.
    data: Option<T>,
}

impl<T> Op<T> {
    #[allow(dead_code)]
    pub(crate) fn new(entry: Entry, data: T) -> Self {
        Self {
            data: Some(data),
            state: State::Initialize(Some(entry)),
        }
    }
    pub(crate) fn take_data(&mut self) -> Option<T> {
        self.data.take()
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        match self.state {
            // We've already deregistere Op.
            State::Complete => (),
            // We have to deregistere Op.
            State::Polled(index) => {
                let handle = crate::runtime::Handle::current();
                handle.inner.driver().io().deregister_op(index);
            }
            State::Initialize(_) => unreachable!(),
        }
    }
}

/// A single CQE entry
pub(crate) struct CqeResult {
    #[allow(dead_code)]
    pub(crate) result: io::Result<u32>,
}

impl From<cqueue::Entry> for CqeResult {
    fn from(cqe: cqueue::Entry) -> Self {
        let res = cqe.result();
        let result = if res >= 0 {
            Ok(res as u32)
        } else {
            Err(io::Error::from_raw_os_error(-res))
        };
        CqeResult { result }
    }
}

pub(crate) trait Completable {
    type Output;
    /// `complete` will be called for cqe's do not have the `more` flag set
    fn complete(self, cqe: CqeResult) -> io::Result<Self::Output>;
}

impl<T: Completable> Future for Op<T> {
    type Output = io::Result<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: `Op` is !Unpin, but we never move any of its fields by
        // projecting `self` into `this` and only mutating through that.
        let this = unsafe { self.get_unchecked_mut() };
        let waker = cx.waker().clone();
        let handle = crate::runtime::Handle::current();
        let driver = &handle.inner.driver().io();

        match &mut this.state {
            State::Initialize(entry_opt) => {
                let entry = entry_opt.take().expect("Initialize must hold an entry");
                let idx = driver.register_op(entry, waker)?;
                this.state = State::Polled(idx);
                Poll::Pending
            }

            State::Polled(idx) => {
                let mut uring = driver.get_uring().lock();
                let lifecycle_slot = &mut uring.ops[*idx];

                // Swap out the old lifecycle so we can match on it
                match mem::replace(lifecycle_slot, Lifecycle::Submitted) {
                    Lifecycle::Submitted => {
                        *lifecycle_slot = Lifecycle::Waiting(waker);
                        Poll::Pending
                    }

                    // Only replace the stored waker if it wouldn't wake the new one
                    Lifecycle::Waiting(prev) if !prev.will_wake(&waker) => {
                        *lifecycle_slot = Lifecycle::Waiting(waker);
                        Poll::Pending
                    }

                    Lifecycle::Waiting(prev) => {
                        *lifecycle_slot = Lifecycle::Waiting(prev);
                        Poll::Pending
                    }

                    Lifecycle::Completed(cqe) => {
                        // Clean up and complete the future
                        uring.ops.remove(*idx);
                        this.state = State::Complete;

                        let data = this
                            .take_data()
                            .expect("Data must be present on completion");
                        Poll::Ready(data.complete(cqe.into()))
                    }

                    Lifecycle::Cancelled => {
                        unreachable!("Cancelled lifecycle should never be seen here");
                    }
                }
            }

            State::Complete => {
                panic!("Future polled after completion");
            }
        }
    }
}
