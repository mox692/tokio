use crate::runtime::context::thread_id;
use io_uring::cqueue;
use io_uring::squeue::Entry;
use std::future::Future;
use std::task::Poll;
use std::task::Waker;
use std::{io, mem};

// TODO: should be placed elsewhere.
#[allow(dead_code)]
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

// TODO: check!!!
unsafe impl Send for Lifecycle {}
unsafe impl Sync for Lifecycle {}

pub(crate) enum State {
    Initialize(Option<Entry>),
    EverPolled(usize), // slab key
    Complete,
}
// TODO: should be placed elsewhere.
pub(crate) struct Op<T> {
    // worker thread that created this Op. Note that there could be a case where
    // this future is sent to another thread. This id is just used for sharding purpose.
    worker_id: u64,
    // state of this Op
    state: State,
    // Per operation data.
    data: Option<T>,
}

impl<T> Op<T> {
    pub(crate) fn new(entry: Entry, data: T) -> Self {
        let uring_len = crate::runtime::Handle::current()
            .inner
            .driver()
            .io()
            .uring_len();

        Self {
            data: Some(data),
            worker_id: thread_id().expect("Failed to get thread ID").as_u64() % uring_len as u64,
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
            // We've already deregistere op. fast path.
            State::Complete => (),
            // We have to deregistere op.
            State::EverPolled(index) => {
                let handle = crate::runtime::Handle::current();
                handle
                    .inner
                    .driver()
                    .io()
                    .deregister_op(self.worker_id, index);
            }
            State::Initialize(_) => unreachable!(),
        }
    }
}

/// A single CQE entry
pub(crate) struct CqeResult {
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
    fn complete(self, cqe: CqeResult) -> Self::Output;
}

impl<T: Completable> Future for Op<T> {
    type Output = T::Output;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // TODO: safety comment
        let this = unsafe { self.get_unchecked_mut() };
        let worker_id = this.worker_id;

        match &mut this.state {
            State::Initialize(entry) => {
                let handle = crate::runtime::Handle::current();

                let index = handle.inner.driver().io().register_op(
                    worker_id,
                    entry.take().unwrap(),
                    cx.waker().clone(),
                );

                this.state = State::EverPolled(index);

                Poll::Pending
            }
            State::EverPolled(index) => {
                let handle = crate::runtime::Handle::current();
                let mut lock = handle
                    .inner
                    .driver()
                    .io()
                    .get_uring(worker_id as usize)
                    .lock();

                let ops = &mut lock.ops;
                let lifecycle = ops.get_mut(*index).unwrap();

                let op = match mem::replace(lifecycle, Lifecycle::Submitted) {
                    Lifecycle::Submitted => {
                        *lifecycle = Lifecycle::Waiting(cx.waker().clone());
                        Poll::Pending
                    }
                    Lifecycle::Waiting(waker) if !waker.will_wake(cx.waker()) => {
                        *lifecycle = Lifecycle::Waiting(cx.waker().clone());
                        Poll::Pending
                    }
                    Lifecycle::Waiting(waker) => {
                        *lifecycle = Lifecycle::Waiting(waker);
                        Poll::Pending
                    }
                    Lifecycle::Cancelled => unreachable!(),
                    Lifecycle::Completed(cqe) => {
                        ops.remove(*index);

                        this.state = State::Complete;

                        Poll::Ready(this.take_data().unwrap().complete(cqe.into()))
                    }
                };
                op
            }

            // TODO: we could reach here if user poll after completion.
            State::Complete => unreachable!(),
        }
    }
}
