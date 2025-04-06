use io_uring::cqueue;
use io_uring::squeue::Entry;

use crate::loom::thread::AccessError;
use crate::task::coop;

use std::cell::Cell;
use std::future::Future;
use std::task::Poll;
use std::{io, mem};

#[cfg(any(feature = "rt", feature = "macros", feature = "time"))]
use crate::util::rand::FastRand;

cfg_rt! {
    mod blocking;
    pub(crate) use blocking::{disallow_block_in_place, try_enter_blocking_region, BlockingRegionGuard};

    mod current;
    pub(crate) use current::{with_current, try_set_current, SetCurrentGuard};

    mod runtime;
    pub(crate) use runtime::{EnterRuntime, enter_runtime};

    mod scoped;
    use scoped::Scoped;

    use crate::runtime::{scheduler, task::Id};

    use std::task::Waker;

    cfg_taskdump! {
        use crate::runtime::task::trace;
    }
}

cfg_rt_multi_thread! {
    mod runtime_mt;
    pub(crate) use runtime_mt::{current_enter_context, exit_runtime};
}

struct Context {
    /// Uniquely identifies the current thread
    #[cfg(feature = "rt")]
    thread_id: Cell<Option<ThreadId>>,

    /// Handle to the runtime scheduler running on the current thread.
    #[cfg(feature = "rt")]
    current: current::HandleCell,

    /// Handle to the scheduler's internal "context"
    #[cfg(feature = "rt")]
    scheduler: Scoped<scheduler::Context>,

    #[cfg(feature = "rt")]
    current_task_id: Cell<Option<Id>>,

    /// Tracks if the current thread is currently driving a runtime.
    /// Note, that if this is set to "entered", the current scheduler
    /// handle may not reference the runtime currently executing. This
    /// is because other runtime handles may be set to current from
    /// within a runtime.
    #[cfg(feature = "rt")]
    runtime: Cell<EnterRuntime>,

    #[cfg(any(feature = "rt", feature = "macros", feature = "time"))]
    rng: Cell<Option<FastRand>>,

    /// Tracks the amount of "work" a task may still do before yielding back to
    /// the scheduler
    budget: Cell<coop::Budget>,

    #[cfg(all(
        tokio_unstable,
        tokio_taskdump,
        feature = "rt",
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
    ))]
    trace: trace::Context,
}

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
        Self {
            data: Some(data),
            worker_id: thread_id().expect("Failed to get thread ID").as_u64(),
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
    pub(crate) flags: u32,
}

impl From<cqueue::Entry> for CqeResult {
    fn from(cqe: cqueue::Entry) -> Self {
        let res = cqe.result();
        let flags = cqe.flags();
        let result = if res >= 0 {
            Ok(res as u32)
        } else {
            Err(io::Error::from_raw_os_error(-res))
        };
        CqeResult { result, flags }
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

tokio_thread_local! {
    static CONTEXT: Context = const {
        Context {
            #[cfg(feature = "rt")]
            thread_id: Cell::new(None),

            // Tracks the current runtime handle to use when spawning,
            // accessing drivers, etc...
            #[cfg(feature = "rt")]
            current: current::HandleCell::new(),

            // Tracks the current scheduler internal context
            #[cfg(feature = "rt")]
            scheduler: Scoped::new(),

            #[cfg(feature = "rt")]
            current_task_id: Cell::new(None),

            // Tracks if the current thread is currently driving a runtime.
            // Note, that if this is set to "entered", the current scheduler
            // handle may not reference the runtime currently executing. This
            // is because other runtime handles may be set to current from
            // within a runtime.
            #[cfg(feature = "rt")]
            runtime: Cell::new(EnterRuntime::NotEntered),

            #[cfg(any(feature = "rt", feature = "macros", feature = "time"))]
            rng: Cell::new(None),

            budget: Cell::new(coop::Budget::unconstrained()),

            #[cfg(all(
                tokio_unstable,
                tokio_taskdump,
                feature = "rt",
                target_os = "linux",
                any(
                    target_arch = "aarch64",
                    target_arch = "x86",
                    target_arch = "x86_64"
                )
            ))]
            trace: trace::Context::new(),
        }
    }
}

#[cfg(any(
    feature = "time",
    feature = "macros",
    all(feature = "sync", feature = "rt")
))]
pub(crate) fn thread_rng_n(n: u32) -> u32 {
    CONTEXT.with(|ctx| {
        let mut rng = ctx.rng.get().unwrap_or_else(FastRand::new);
        let ret = rng.fastrand_n(n);
        ctx.rng.set(Some(rng));
        ret
    })
}

pub(crate) fn budget<R>(f: impl FnOnce(&Cell<coop::Budget>) -> R) -> Result<R, AccessError> {
    CONTEXT.try_with(|ctx| f(&ctx.budget))
}

cfg_rt! {
    use crate::runtime::ThreadId;

    pub(crate) fn thread_id() -> Result<ThreadId, AccessError> {
        CONTEXT.try_with(|ctx| {
            match ctx.thread_id.get() {
                Some(id) => id,
                None => {
                    let id = ThreadId::next();
                    ctx.thread_id.set(Some(id));
                    id
                }
            }
        })
    }

    pub(crate) fn set_current_task_id(id: Option<Id>) -> Option<Id> {
        CONTEXT.try_with(|ctx| ctx.current_task_id.replace(id)).unwrap_or(None)
    }

    pub(crate) fn current_task_id() -> Option<Id> {
        CONTEXT.try_with(|ctx| ctx.current_task_id.get()).unwrap_or(None)
    }

    #[track_caller]
    pub(crate) fn defer(waker: &Waker) {
        with_scheduler(|maybe_scheduler| {
            if let Some(scheduler) = maybe_scheduler {
                scheduler.defer(waker);
            } else {
                // Called from outside of the runtime, immediately wake the
                // task.
                waker.wake_by_ref();
            }
        });
    }

    pub(super) fn set_scheduler<R>(v: &scheduler::Context, f: impl FnOnce() -> R) -> R {
        CONTEXT.with(|c| c.scheduler.set(v, f))
    }

    #[track_caller]
    pub(super) fn with_scheduler<R>(f: impl FnOnce(Option<&scheduler::Context>) -> R) -> R {
        let mut f = Some(f);
        CONTEXT.try_with(|c| {
            let f = f.take().unwrap();
            if matches!(c.runtime.get(), EnterRuntime::Entered { .. }) {
                c.scheduler.with(f)
            } else {
                f(None)
            }
        })
            .unwrap_or_else(|_| (f.take().unwrap())(None))
    }

    cfg_taskdump! {
        /// SAFETY: Callers of this function must ensure that trace frames always
        /// form a valid linked list.
        pub(crate) unsafe fn with_trace<R>(f: impl FnOnce(&trace::Context) -> R) -> Option<R> {
            CONTEXT.try_with(|c| f(&c.trace)).ok()
        }
    }
}
