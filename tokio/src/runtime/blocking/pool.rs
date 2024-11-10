//! Thread pool for blocking operations

use crossbeam::queue::SegQueue;

use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::{Arc, Condvar, Mutex, RwLock};
use crate::loom::thread;
use crate::runtime::blocking::schedule::BlockingSchedule;
use crate::runtime::blocking::{shutdown, BlockingTask};
use crate::runtime::builder::ThreadNameFn;
use crate::runtime::task::{self, JoinHandle};
use crate::runtime::{Builder, Callback, Handle, BOX_FUTURE_THRESHOLD};
use crate::util::metric_atomics::MetricAtomicUsize;
use crate::util::trace::{blocking_task, SpawnMeta};

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

pub(crate) struct BlockingPool {
    spawner: Spawner,
    shutdown_rx: shutdown::Receiver,
}

#[derive(Clone)]
pub(crate) struct Spawner {
    inner: Arc<Inner>,
}

#[derive(Default)]
pub(crate) struct SpawnerMetrics {
    num_threads: MetricAtomicUsize,
    num_idle_threads: MetricAtomicUsize,
    queue_depth: MetricAtomicUsize,
}

impl SpawnerMetrics {
    fn num_threads(&self) -> usize {
        self.num_threads.load(Ordering::Relaxed)
    }

    fn num_idle_threads(&self) -> usize {
        self.num_idle_threads.load(Ordering::Relaxed)
    }

    cfg_unstable_metrics! {
        fn queue_depth(&self) -> usize {
            self.queue_depth.load(Ordering::Relaxed)
        }
    }

    fn inc_num_threads(&self) {
        self.num_threads.increment();
    }

    fn dec_num_threads(&self) {
        self.num_threads.decrement();
    }

    fn inc_num_idle_threads(&self) {
        self.num_idle_threads.increment();
    }

    fn dec_num_idle_threads(&self) -> usize {
        self.num_idle_threads.decrement()
    }

    fn inc_queue_depth(&self) {
        self.queue_depth.increment();
    }

    fn dec_queue_depth(&self) {
        self.queue_depth.decrement();
    }
}

struct Inner {
    /// State shared between worker threads.
    shared: Shared,

    queue: SegQueue<Task>,

    /// Pool threads wait on this.
    condvar: Condvar,

    /// just used for condvar
    mutex: Mutex<()>,

    /// Spawned threads use this name.
    thread_name: ThreadNameFn,

    /// Spawned thread stack size.
    stack_size: Option<usize>,

    /// Call after a thread starts.
    after_start: Option<Callback>,

    /// Call before a thread stops.
    before_stop: Option<Callback>,

    // Maximum number of threads.
    thread_cap: usize,

    // Customizable wait timeout.
    keep_alive: Duration,

    // Metrics about the pool.
    metrics: SpawnerMetrics,
}

struct Shared {
    num_notify: AtomicU32,
    shutdown: AtomicBool,
    shutdown_tx: RwLock<Option<shutdown::Sender>>,
    /// Prior to shutdown, we clean up `JoinHandles` by having each timed-out
    /// thread join on the previous timed-out thread. This is not strictly
    /// necessary but helps avoid Valgrind false positives, see
    /// <https://github.com/tokio-rs/tokio/commit/646fbae76535e397ef79dbcaacb945d4c829f666>
    /// for more information.
    last_exiting_thread: Mutex<Option<thread::JoinHandle<()>>>,
    /// This holds the `JoinHandles` for all running threads; on shutdown, the thread
    /// calling shutdown handles joining on these.
    worker_threads: Mutex<HashMap<usize, thread::JoinHandle<()>>>,
    /// This is a counter used to iterate `worker_threads` in a consistent order (for loom's
    /// benefit).
    worker_thread_index: AtomicUsize,
}

pub(crate) struct Task {
    task: task::UnownedTask<BlockingSchedule>,
    mandatory: Mandatory,
}

#[derive(PartialEq, Eq)]
pub(crate) enum Mandatory {
    #[cfg_attr(not(fs), allow(dead_code))]
    Mandatory,
    NonMandatory,
}

pub(crate) enum SpawnError {
    /// Pool is shutting down and the task was not scheduled
    ShuttingDown,
    /// There are no worker threads available to take the task
    /// and the OS failed to spawn a new one
    NoThreads(io::Error),
}

impl From<SpawnError> for io::Error {
    fn from(e: SpawnError) -> Self {
        match e {
            SpawnError::ShuttingDown => {
                io::Error::new(io::ErrorKind::Other, "blocking pool shutting down")
            }
            SpawnError::NoThreads(e) => e,
        }
    }
}

impl Task {
    pub(crate) fn new(task: task::UnownedTask<BlockingSchedule>, mandatory: Mandatory) -> Task {
        Task { task, mandatory }
    }

    fn run(self) {
        self.task.run();
    }

    fn shutdown_or_run_if_mandatory(self) {
        match self.mandatory {
            Mandatory::NonMandatory => self.task.shutdown(),
            Mandatory::Mandatory => self.task.run(),
        }
    }
}

const KEEP_ALIVE: Duration = Duration::from_secs(10);

/// Runs the provided function on an executor dedicated to blocking operations.
/// Tasks will be scheduled as non-mandatory, meaning they may not get executed
/// in case of runtime shutdown.
#[track_caller]
#[cfg_attr(target_os = "wasi", allow(dead_code))]
pub(crate) fn spawn_blocking<F, R>(func: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let rt = Handle::current();
    rt.spawn_blocking(func)
}

cfg_fs! {
    #[cfg_attr(any(
        all(loom, not(test)), // the function is covered by loom tests
        test
    ), allow(dead_code))]
    /// Runs the provided function on an executor dedicated to blocking
    /// operations. Tasks will be scheduled as mandatory, meaning they are
    /// guaranteed to run unless a shutdown is already taking place. In case a
    /// shutdown is already taking place, `None` will be returned.
    pub(crate) fn spawn_mandatory_blocking<F, R>(func: F) -> Option<JoinHandle<R>>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let rt = Handle::current();
        rt.inner.blocking_spawner().spawn_mandatory_blocking(&rt, func)
    }
}

// ===== impl BlockingPool =====

impl BlockingPool {
    pub(crate) fn new(builder: &Builder, thread_cap: usize) -> BlockingPool {
        let (shutdown_tx, shutdown_rx) = shutdown::channel();
        let keep_alive = builder.keep_alive.unwrap_or(KEEP_ALIVE);

        BlockingPool {
            spawner: Spawner {
                inner: Arc::new(Inner {
                    shared: Shared {
                        num_notify: AtomicU32::new(0),
                        shutdown: AtomicBool::new(false),
                        shutdown_tx: RwLock::new(Some(shutdown_tx)),
                        last_exiting_thread: Mutex::new(None),
                        worker_threads: Mutex::new(HashMap::new()),
                        worker_thread_index: AtomicUsize::new(0),
                    },
                    queue: SegQueue::new(),
                    condvar: Condvar::new(),
                    mutex: Mutex::new(()),
                    thread_name: builder.thread_name.clone(),
                    stack_size: builder.thread_stack_size,
                    after_start: builder.after_start.clone(),
                    before_stop: builder.before_stop.clone(),
                    thread_cap,
                    keep_alive,
                    metrics: SpawnerMetrics::default(),
                }),
            },
            shutdown_rx,
        }
    }

    pub(crate) fn spawner(&self) -> &Spawner {
        &self.spawner
    }

    pub(crate) fn shutdown(&mut self, timeout: Option<Duration>) {
        // The function can be called multiple times. First, by explicitly
        // calling `shutdown` then by the drop handler calling `shutdown`. This
        // prevents shutting down twice.
        if self.spawner.inner.shared.shutdown.load(Ordering::Acquire) {
            return;
        }

        self.spawner
            .inner
            .shared
            .shutdown
            .store(true, Ordering::Release);
        let mut guard = self.spawner.inner.shared.shutdown_tx.write();
        *guard = None;
        drop(guard);
        self.spawner.inner.condvar.notify_all();

        let mut guard = self.spawner.inner.shared.last_exiting_thread.lock();
        let mut last_exiting_thread = guard.take();
        let last_exited_thread = std::mem::take(&mut last_exiting_thread);
        drop(guard);
        let workers: Vec<(usize, thread::JoinHandle<()>)> = self
            .spawner
            .inner
            .shared
            .worker_threads
            .lock()
            .drain()
            .collect();

        if self.shutdown_rx.wait(timeout) {
            let _ = last_exited_thread.map(thread::JoinHandle::join);

            // Loom requires that execution be deterministic, so sort by thread ID before joining.
            // (HashMaps use a randomly-seeded hash function, so the order is nondeterministic)
            #[cfg(loom)]
            let workers: Vec<(usize, thread::JoinHandle<()>)> = {
                let mut workers: Vec<_> = workers.into_iter().collect();
                workers.sort_by_key(|(id, _)| *id);
                workers
            };

            for (_id, handle) in workers {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for BlockingPool {
    fn drop(&mut self) {
        self.shutdown(None);
    }
}

impl fmt::Debug for BlockingPool {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BlockingPool").finish()
    }
}

// ===== impl Spawner =====

impl Spawner {
    #[track_caller]
    pub(crate) fn spawn_blocking<F, R>(&self, rt: &Handle, func: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let fn_size = std::mem::size_of::<F>();
        let (join_handle, spawn_result) = if fn_size > BOX_FUTURE_THRESHOLD {
            self.spawn_blocking_inner(
                Box::new(func),
                Mandatory::NonMandatory,
                SpawnMeta::new_unnamed(fn_size),
                rt,
            )
        } else {
            self.spawn_blocking_inner(
                func,
                Mandatory::NonMandatory,
                SpawnMeta::new_unnamed(fn_size),
                rt,
            )
        };

        match spawn_result {
            Ok(()) => join_handle,
            // Compat: do not panic here, return the join_handle even though it will never resolve
            Err(SpawnError::ShuttingDown) => join_handle,
            Err(SpawnError::NoThreads(e)) => {
                panic!("OS can't spawn worker thread: {}", e)
            }
        }
    }

    cfg_fs! {
        #[track_caller]
        #[cfg_attr(any(
            all(loom, not(test)), // the function is covered by loom tests
            test
        ), allow(dead_code))]
        pub(crate) fn spawn_mandatory_blocking<F, R>(&self, rt: &Handle, func: F) -> Option<JoinHandle<R>>
        where
            F: FnOnce() -> R + Send + 'static,
            R: Send + 'static,
        {
            let fn_size = std::mem::size_of::<F>();
            let (join_handle, spawn_result) = if fn_size > BOX_FUTURE_THRESHOLD {
                self.spawn_blocking_inner(
                    Box::new(func),
                    Mandatory::Mandatory,
                    SpawnMeta::new_unnamed(fn_size),
                    rt,
                )
            } else {
                self.spawn_blocking_inner(
                    func,
                    Mandatory::Mandatory,
                    SpawnMeta::new_unnamed(fn_size),
                    rt,
                )
            };

            if spawn_result.is_ok() {
                Some(join_handle)
            } else {
                None
            }
        }
    }

    #[track_caller]
    pub(crate) fn spawn_blocking_inner<F, R>(
        &self,
        func: F,
        is_mandatory: Mandatory,
        spawn_meta: SpawnMeta<'_>,
        rt: &Handle,
    ) -> (JoinHandle<R>, Result<(), SpawnError>)
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let id = task::Id::next();
        let fut =
            blocking_task::<F, BlockingTask<F>>(BlockingTask::new(func), spawn_meta, id.as_u64());

        let (task, handle) = task::unowned(fut, BlockingSchedule::new(rt), id);

        let spawned = self.spawn_task(Task::new(task, is_mandatory), rt);
        (handle, spawned)
    }

    fn spawn_task(&self, task: Task, rt: &Handle) -> Result<(), SpawnError> {
        let shared = &self.inner.shared;

        if shared.shutdown.load(Ordering::Acquire) {
            // Shutdown the task: it's fine to shutdown this task (even if
            // mandatory) because it was scheduled after the shutdown of the
            // runtime began.
            task.task.shutdown();

            // no need to even push this task; it would never get picked up
            return Err(SpawnError::ShuttingDown);
        }

        self.inner.queue.push(task);
        self.inner.metrics.inc_queue_depth();

        if self.inner.metrics.num_idle_threads() == 0 {
            // No threads are able to process the task.

            if self.inner.metrics.num_threads() == self.inner.thread_cap {
                // At max number of threads
            } else {
                let shutdown_tx = shared.shutdown_tx.read();
                assert!(shutdown_tx.is_some());
                let shutdown_tx = shutdown_tx.clone();

                if let Some(shutdown_tx) = shutdown_tx {
                    let id = shared.worker_thread_index.load(Ordering::Acquire);

                    match self.spawn_thread(shutdown_tx, rt, id) {
                        Ok(handle) => {
                            self.inner.metrics.inc_num_threads();
                            shared.worker_thread_index.fetch_add(1, Ordering::Relaxed);
                            let mut guard = shared.worker_threads.lock();
                            guard.insert(id, handle);
                            drop(guard);
                        }
                        Err(ref e)
                            if is_temporary_os_thread_error(e)
                                && self.inner.metrics.num_threads() > 0 =>
                        {
                            // OS temporarily failed to spawn a new thread.
                            // The task will be picked up eventually by a currently
                            // busy thread.
                        }
                        Err(e) => {
                            // The OS refused to spawn the thread and there is no thread
                            // to pick up the task that has just been pushed to the queue.
                            return Err(SpawnError::NoThreads(e));
                        }
                    }
                }
            }
        } else {
            // Notify an idle worker thread. The notification counter
            // is used to count the needed amount of notifications
            // exactly. Thread libraries may generate spurious
            // wakeups, this counter is used to keep us in a
            // consistent state.
            self.inner.metrics.dec_num_idle_threads();
            shared.num_notify.fetch_add(1, Ordering::Relaxed);
            self.inner.condvar.notify_one();
        }

        Ok(())
    }

    fn spawn_thread(
        &self,
        shutdown_tx: shutdown::Sender,
        rt: &Handle,
        id: usize,
    ) -> io::Result<thread::JoinHandle<()>> {
        let mut builder = thread::Builder::new().name((self.inner.thread_name)());

        if let Some(stack_size) = self.inner.stack_size {
            builder = builder.stack_size(stack_size);
        }

        let rt = rt.clone();

        builder.spawn(move || {
            // Only the reference should be moved into the closure
            let _enter = rt.enter();
            rt.inner.blocking_spawner().inner.run(id);
            drop(shutdown_tx);
        })
    }
}

cfg_unstable_metrics! {
    impl Spawner {
        pub(crate) fn num_threads(&self) -> usize {
            self.inner.metrics.num_threads()
        }

        pub(crate) fn num_idle_threads(&self) -> usize {
            self.inner.metrics.num_idle_threads()
        }

        pub(crate) fn queue_depth(&self) -> usize {
            self.inner.metrics.queue_depth()
        }
    }
}

// Tells whether the error when spawning a thread is temporary.
#[inline]
fn is_temporary_os_thread_error(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::WouldBlock)
}

impl Inner {
    fn run(&self, worker_thread_id: usize) {
        if let Some(f) = &self.after_start {
            f();
        }

        let shared = &self.shared;
        let mut join_on_thread = None;

        'main: loop {
            // BUSY
            while let Some(task) = self.queue.pop() {
                self.metrics.dec_queue_depth();
                task.run();
            }

            // IDLE
            self.metrics.inc_num_idle_threads();

            while !shared.shutdown.load(Ordering::Acquire) {
                let mut guard = self.mutex.lock();
                let lock_result = self.condvar.wait_timeout(guard, self.keep_alive).unwrap();

                let timeout_result = lock_result.1;

                if shared.num_notify.load(Ordering::Acquire) != 0 {
                    // We have received a legitimate wakeup,
                    // acknowledge it by decrementing the counter
                    // and transition to the BUSY state.
                    shared.num_notify.fetch_sub(1, Ordering::Release);
                    break;
                }

                // Even if the condvar "timed out", if the pool is entering the
                // shutdown phase, we want to perform the cleanup logic.
                if !shared.shutdown.load(Ordering::Acquire) && timeout_result.timed_out() {
                    // We'll join the prior timed-out thread's JoinHandle after dropping the lock.
                    // This isn't done when shutting down, because the thread calling shutdown will
                    // handle joining everything.
                    let mut guard = shared.worker_threads.lock();
                    let my_handle = guard.remove(&worker_thread_id);
                    drop(guard);
                    let mut guard = shared.last_exiting_thread.lock();
                    let mut last_exiting_thread = guard.take();
                    join_on_thread = std::mem::replace(&mut last_exiting_thread, my_handle);
                    drop(guard);

                    break 'main;
                }

                // Spurious wakeup detected, go back to sleep.
            }

            if shared.shutdown.load(Ordering::Acquire) {
                // Drain the queue
                while let Some(task) = self.queue.pop() {
                    self.metrics.dec_queue_depth();

                    task.shutdown_or_run_if_mandatory();
                }

                // Work was produced, and we "took" it (by decrementing num_notify).
                // This means that num_idle was decremented once for our wakeup.
                // But, since we are exiting, we need to "undo" that, as we'll stay idle.
                self.metrics.inc_num_idle_threads();
                // NOTE: Technically we should also do num_notify++ and notify again,
                // but since we're shutting down anyway, that won't be necessary.
                break;
            }
        }

        // Thread exit
        self.metrics.dec_num_threads();

        // num_idle should now be tracked exactly, panic
        // with a descriptive message if it is not the
        // case.
        let prev_idle = self.metrics.dec_num_idle_threads();
        assert!(
            prev_idle >= self.metrics.num_idle_threads(),
            "num_idle_threads underflowed on thread exit"
        );

        if shared.shutdown.load(Ordering::SeqCst) && self.metrics.num_threads() == 0 {
            self.condvar.notify_one();
        }

        drop(shared);

        if let Some(f) = &self.before_stop {
            f();
        }

        if let Some(handle) = join_on_thread {
            let _ = handle.join();
        }
    }
}

impl fmt::Debug for Spawner {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("blocking::Spawner").finish()
    }
}
