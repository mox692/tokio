This proposal is intended to initiate discussion and is subject to future revisions.

# Summary

* This proposal suggests adding support for `io_uring` in Tokio's file API.
* Initially, the goal is to transparently replace the file API backend with io_uring from a thread pool. Advanced features such as registered fds or registered buffers will be addressed in separate proposal.
* The application of io_uring to network I/O is outside the scope of this proposal.
* The implementation will happen incrementally.

# Motivation

The current `File` API uses `spawn_blocking` for each operation, which runs tasks on a thread pool. On Linux, however, using `io_uring` for file I/O can potentially bring the following perf improvements:

* Fewer system calls
* Reduction in thread creation per operation

For broader background information that is related to introducing io_uring in tokio, refer to the [tokio-uring DESIGN document](https://github.com/tokio-rs/tokio-uring/blob/master/DESIGN.md#motivation).

# Guide-level explanation

### Overview

Currently, Tokio uses `epoll` for IO operation. Since file descriptors created by `io_uring` can be registered with `epoll`, it's possible to detect completion events via `epoll_ctl`. This mechanism allows for some level of coexistence between `epoll` and `io_uring`.

At a high level, the following changes will likely be required:

* Modify the driver to:
  * Register the `io_uring` file descriptor
  * Submit operations to the submission queue when performing file operations
  * Wake tasks upon completion via `io_uring`
* Introduce a new `Future` type to represent `io_uring` operations (similar to `tokio-uring`'s `Op<T>`)
* Modify the `fs` module to use `io_uring` internally

I think these changes can be achievable without breaking changes.

### API Surface

For now, the goal should be to replace the backend of the existing file API with `io_uring`. Therefore, the public API will remain unchanged. As a result, users can continue using the current file APIs and transparently benefit from `io_uring`.

### Opt-in options

While the feature is unstable, it may be useful to allow opting in to use `io_uring`. This would help in gradually transitioning the API and would also benefit users who, for example, run on older Linux kernels and prefer to continue using the thread-pool implementation even after stabilization.  

There are some options for this.

**Add config options to `OpenOptions`?**  
One idea would be to add a mode-selection option (e.g., `set_mode`) to `fs::OpenOptions`.

```rust
let file = OpenOptions::new()
    .read(true)
    // A file object that is created by `set_mode(Mode::Uring(...))` will use
    // io_uring to perform IO.
    .set_mode(Mode::Uring(uring_config)) // **NEW**
    .open(&path)
    .await;

file.read(&mut buf).await; // this read will use io_uring
```

* pros
  * It will also be possible to configure a dedicated setting for io_uring.
* cons
  * This approach requires adding some new public APIs that is only used in linux.
  * It cannot be applied to one-shot operations like `tokio::fs::write()` or `tokio::fs::read()`.


**Support dedicated cfg: `--tokio_uring_fs`?**  
Similar to the existing `taskdump` cfg, a new compile-time cfg option could be introduced. Compared to the runtime opt-in, this has the advantage of supporting one-shot APIs (`tokio::fs::write()`, `tokio::fs::read()` etc). However, it removes the ability to switch implementations at runtime.

* pros
  * It's possible to switch between `spawn_blocking` and `io_uring` even when using the same source.
  * It supports one-shot APIs (`tokio::fs::write()`, `tokio::fs::read()` etc)
* cons
  * No ability to switch implementations between thread pool and uring at runtime.
  * No opt-out way when io_uring is going to be default


# Details

Below is a more concrete implementation side details for how to achieve the goals described above.

### Register Uring File Descriptor

By registering the `io_uring` file descriptor with `epoll`, we can receive completion events via `epoll_ctl`. This can be done at the time of the first file I/O or during runtime initialization using mio. For the initial implementation, we can start with a dummy token, e.g. `TOKEN_WAKEUP`.

The code for uring fd initialization would look like this:

```rust
pub(crate) fn add_uring_source(
    &self,
    source: &mut impl mio::event::Source,
) -> io::Result<()> {
    // Register uringfd to mio
    self.registry
        .register(source, TOKEN_WAKEUP, Interest::READABLE)
}

// Register uringfd
let uringfd = IoUring::new(num_entry).unwrap();
let mut source = SourceFd(&uringfd);
add_uring_source(&mut source);
```

### Uring Tasks

When the file API is invoked, an associated `UringFuture` is created internally. The lifecycle of these futures is as follows:

* **Submitted**: When the future is created, it starts in this state. It pushes its operation onto the submission queue. It also registers itself in a shared data structure that tracks in-flight operations, then transitions to `Pending`.
* **Pending**: The operation has not yet completed. The future holds a waker for later use.
* **Completed**: When the operation completes and the driver wakes the task, the future transitions to this state and returns the result to the user.

The design of these futures will largely follow that of [tokio-uring](https://github.com/tokio-rs/tokio-uring/blob/7761222aa7f4bd48c559ca82e9535d47aac96d53/src/runtime/driver/op/mod.rs#L160-L177).

### Driver

Once the `uringfd` is registered with `epoll`, completion of submitted operations will cause `epoll` to return. After processing regular `epoll` events, the driver can also handle `cqe` entries to wake tasks associated with `io_uring`.

The driver maintains a list of in-flight operations and can identify which operation completed using the `userdata` field in the `cqe`.

A pseudocode example from within the driver:

```rust
// tokio/src/runtime/io/driver.rs

// Polling events ...
self.poll.poll(events, max_wait);

// Process epoll events
for event in events.iter() {
}

/* NEW */
// Process uring events
for cqe in cq.iter() {
    // Process uring events
    let index = cqe.userdata();
    // Look up which operation has finished
    let operation = operation_list.get(index);
    operation.wake();
}
```

### Multi thread

For the multi-threaded runtime, there are multiple strategies for managing rings:

The simplest approach is to maintain a single global ring. This is easy to implement but could become a bottleneck as the number of threads increases.

Alternatively, we can reduce contention by sharding the ring (i.e.assigning a dedicated ring per worker thread). There are several potential approaches to this:

* Store a data structure dedicated to io_uring inside mio's `Token`, and keep the shard_id (index of the worker) in that data structure.
* Give each worker thread its own io_uring driver (submissions and completions happen within the worker thread.)

But we could probably start with a single global ring, and then work on sharding as a follow-up.

# Drawbacks

* Since we will support this incrementally, certain workloads might not benefit much in terms of performance until batching or sharding rings are well supported.
* If we adopt a strategy where the driver wakes the epoll event first and then wakes the uring, an implicit prioritization may be introduced into task scheduling. (Tasks triggered by epoll events may be executed first.)

# Alternatives
**Waiting for epoll events with io_uring**  

The integration of epoll and io_uring is also possible by having io_uring wait on epoll events (e.g., using `IORING_OP_POLL_ADD`). However, this would require large changes of the existing epoll-based runtime.

**Defining a dedicated File object for io_uring**  

Instead of replacing the I/O backend, we could provide a new File object dedicated to io_uring. This approach would require users to explicitly replace the File object, which is not ideal. Furthermore, it would necessitate maintaining a Linux-specific type.

**Creating a Tokio task that polls uring tasks**

This is the strategy that tokio-uring uses. However, unlike tokio-uring, this proposal has direct access to the Tokio runtime driver, so there's no need to create a dedicated task for that purpose.

# Prior art

### tokio-uring
As prior work, there is the `tokio-uring` project. The differences between that project and this proposal are:

* Supports not only file I/O but also network I/O
* Supports advanced features such as kernel-registered buffers
* Based on the current-thread runtime

However, some parts, such as the `Future` related to `Operation` (`Op`), are likely to be inherited.

# Unresolved questions

**How to provide a flag during the unstable period**  
(Although discussed above,) it is still unclear how users should opt-in to io_uring during the transition period.

**Intelligent batching logic for submission**  
To maximize io_uring performance, it is important to make effective use of batching at submission. The best strategy for batching within Tokio's event loop is still unclear.  
But, this proposal aims to align on a high-level design, so the detailed implementation strategy for batching will be handled in a separate issue or PR.

**How to manage the ring in a multi-threaded runtime**  
Detailed implementation strategies for sharding rings across threads in a multithreaded will also continue to be discussed in a separate issue or PR.

# Future work

I think this proposal can be achieved incrementally, as follows:

1. Add minimal io_uring file API support for the current-thread runtime
   * Add uring support as an opt-in option (using a dedicated `cfg` or `OpenOptions`)
   * Initially support only some key APIs (e.g., `fs::read()`, `fs::write()` only)
2. Multi-threaded runtime support
   * For simplicity, we could start with a single global ring
3. Further improvements, such as:
   * Sharding rings in the multi-threaded runtime (one ring per thread)
   * Expanding the use of io_uring to other filesystem APIs (e.g., `fs::File`)
   * Smarter batching logic for submission
   * Exploring the possibilities of using advanced features, such as registered buffers or registered files
4. Use io_uring as the default for `File::new`, `fs::read`, `fs::write`, etc.
5. Stabilize (remove `tokio_unstable`)
