use crate::io::interest::Interest;
use crate::runtime::io::Registration;
use crate::runtime::scheduler;

use mio::event::Source;
use std::fmt;
use std::io;
use std::ops::Deref;
use std::panic::{RefUnwindSafe, UnwindSafe};

cfg_io_driver! {
    /// Associates an I/O resource that implements the [`std::io::Read`] and/or
    /// [`std::io::Write`] traits with the reactor that drives it.
    ///
    /// `PollEvented` uses [`Registration`] internally to take a type that
    /// implements [`mio::event::Source`] as well as [`std::io::Read`] and/or
    /// [`std::io::Write`] and associate it with a reactor that will drive it.
    ///
    /// Once the [`mio::event::Source`] type is wrapped by `PollEvented`, it can be
    /// used from within the future's execution model. As such, the
    /// `PollEvented` type provides [`AsyncRead`] and [`AsyncWrite`]
    /// implementations using the underlying I/O resource as well as readiness
    /// events provided by the reactor.
    ///
    /// **Note**: While `PollEvented` is `Sync` (if the underlying I/O type is
    /// `Sync`), the caller must ensure that there are at most two tasks that
    /// use a `PollEvented` instance concurrently. One for reading and one for
    /// writing. While violating this requirement is "safe" from a Rust memory
    /// model point of view, it will result in unexpected behavior in the form
    /// of lost notifications and tasks hanging.
    ///
    /// ## Readiness events
    ///
    /// Besides just providing [`AsyncRead`] and [`AsyncWrite`] implementations,
    /// this type also supports access to the underlying readiness event stream.
    /// While similar in function to what [`Registration`] provides, the
    /// semantics are a bit different.
    ///
    /// Two functions are provided to access the readiness events:
    /// [`poll_read_ready`] and [`poll_write_ready`]. These functions return the
    /// current readiness state of the `PollEvented` instance. If
    /// [`poll_read_ready`] indicates read readiness, immediately calling
    /// [`poll_read_ready`] again will also indicate read readiness.
    ///
    /// When the operation is attempted and is unable to succeed due to the I/O
    /// resource not being ready, the caller must call [`clear_readiness`].
    /// This clears the readiness state until a new readiness event is received.
    ///
    /// This allows the caller to implement additional functions. For example,
    /// [`TcpListener`] implements `poll_accept` by using [`poll_read_ready`] and
    /// [`clear_readiness`].
    ///
    /// ## Platform-specific events
    ///
    /// `PollEvented` also allows receiving platform-specific `mio::Ready` events.
    /// These events are included as part of the read readiness event stream. The
    /// write readiness event stream is only for `Ready::writable()` events.
    ///
    /// [`AsyncRead`]: crate::io::AsyncRead
    /// [`AsyncWrite`]: crate::io::AsyncWrite
    /// [`TcpListener`]: crate::net::TcpListener
    /// [`clear_readiness`]: Registration::clear_readiness
    /// [`poll_read_ready`]: Registration::poll_read_ready
    /// [`poll_write_ready`]: Registration::poll_write_ready
    ///
    /// [`std::io::Read`] および/または [`std::io::Write`] トレイトを実装する I/O リソースを、これを駆動するリアクターに関連付けます。
    ///
    /// `PollEvented` は内部的に [`Registration`] を使用して、[`mio::event::Source`] および [`std::io::Read`] および/または [`std::io::Write`] を実装する型を取得し、
    /// それを駆動するリアクターに関連付けます。
    ///
    /// [`mio::event::Source`] 型が `PollEvented` にラップされると、将来の実行モデル内で使用できるようになります。
    /// このため、`PollEvented` 型は、基盤となる I/O リソースおよびリアクターが提供する準備完了イベントを使用して、[`AsyncRead`] および [`AsyncWrite`] 実装を提供します。
    ///
    /// **注意**:
    /// `PollEvented` は（基盤となる I/O 型が `Sync` である場合）`Sync` ですが、呼び出し側は `PollEvented`
    /// インスタンスを同時に使用するタスクが最大で 2 つであることを確認する必要があります。1つは読み取り用、もう1つは書き込み用です。
    /// この要件を満たさない場合、Rust のメモリモデルの観点からは「安全」ですが、通知が失われたりタスクがハングしたりする形で予期しない動作が発生します。
    ///
    /// ## 準備完了イベント
    ///
    /// [`AsyncRead`] および [`AsyncWrite`] 実装を提供するだけでなく、この型は基盤となる準備完了イベントストリームへのアクセスもサポートしています。
    /// これは [`Registration`] が提供するものと機能は似ていますが、セマンティクスが少し異なります。
    /// 準備完了イベントにアクセスするための関数が 2 つ提供されています：[`poll_read_ready`] と [`poll_write_ready`] です。これらの関数は `PollEvented` インスタンスの現在の準備状態を返します。もし [`poll_read_ready`] が読み取りの準備完了を示す場合、再度 [`poll_read_ready`] を呼び出しても読み取りの準備完了を示します。
    /// 操作が試みられ、I/O リソースが準備完了でないために成功できない場合、呼び出し側は [`clear_readiness`] を呼び出す必要があります。
    /// これにより、新しい準備完了イベントが受信されるまで準備状態がクリアされます。
    ///
    /// これにより、呼び出し側は追加の関数を実装できます。例えば、[`TcpListener`] は [`poll_read_ready`] と [`clear_readiness`] を使用して `poll_accept` を実装します。
    ///
    /// ## プラットフォーム固有のイベント
    ///
    /// `PollEvented` はプラットフォーム固有の `mio::Ready` イベントの受信も可能にします。
    /// これらのイベントは読み取り準備完了イベントストリームの一部として含まれます。書き込み準備完了イベントストリームには `Ready::writable()` イベントのみが含まれます。
    ///
    /// [`AsyncRead`]: crate::io::AsyncRead
    /// [`AsyncWrite`]: crate::io::AsyncWrite
    /// [`TcpListener`]: crate::net::TcpListener
    /// [`clear_readiness`]: Registration::clear_readiness
    /// [`poll_read_ready`]: Registration::poll_read_ready
    pub(crate) struct PollEvented<E: Source> {
        io: Option<E>,
        registration: Registration,
    }
}

// ===== impl PollEvented =====

impl<E: Source> PollEvented<E> {
    /// Creates a new `PollEvented` associated with the default reactor.
    ///
    /// The returned `PollEvented` has readable and writable interests. For more control, use
    /// [`Self::new_with_interest`].
    ///
    /// # Panics
    ///
    /// This function panics if thread-local runtime is not set.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    #[track_caller]
    #[cfg_attr(feature = "signal", allow(unused))]
    pub(crate) fn new(io: E) -> io::Result<Self> {
        PollEvented::new_with_interest(io, Interest::READABLE | Interest::WRITABLE)
    }

    /// Creates a new `PollEvented` associated with the default reactor, for
    /// specific `Interest` state. `new_with_interest` should be used over `new`
    /// when you need control over the readiness state, such as when a file
    /// descriptor only allows reads. This does not add `hup` or `error` so if
    /// you are interested in those states, you will need to add them to the
    /// readiness state passed to this function.
    ///
    /// # Panics
    ///
    /// This function panics if thread-local runtime is not set.
    ///
    /// The runtime is usually set implicitly when this function is called from
    /// a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter)
    /// function.
    #[track_caller]
    #[cfg_attr(feature = "signal", allow(unused))]
    pub(crate) fn new_with_interest(io: E, interest: Interest) -> io::Result<Self> {
        Self::new_with_interest_and_handle(io, interest, scheduler::Handle::current())
    }

    #[track_caller]
    pub(crate) fn new_with_interest_and_handle(
        mut io: E,
        interest: Interest,
        handle: scheduler::Handle,
    ) -> io::Result<Self> {
        let registration = Registration::new_with_interest_and_handle(&mut io, interest, handle)?;
        Ok(Self {
            io: Some(io),
            registration,
        })
    }

    /// Returns a reference to the registration.
    #[cfg(feature = "net")]
    pub(crate) fn registration(&self) -> &Registration {
        &self.registration
    }

    /// Deregisters the inner io from the registration and returns a Result containing the inner io.
    #[cfg(any(feature = "net", feature = "process"))]
    pub(crate) fn into_inner(mut self) -> io::Result<E> {
        let mut inner = self.io.take().unwrap(); // As io shouldn't ever be None, just unwrap here.
        self.registration.deregister(&mut inner)?;
        Ok(inner)
    }

    #[cfg(all(feature = "process", target_os = "linux"))]
    pub(crate) fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.registration
            .poll_read_ready(cx)
            .map_err(io::Error::from)
            .map_ok(|_| ())
    }

    /// Re-register under new runtime with `interest`.
    #[cfg(all(feature = "process", target_os = "linux"))]
    pub(crate) fn reregister(&mut self, interest: Interest) -> io::Result<()> {
        let io = self.io.as_mut().unwrap(); // As io shouldn't ever be None, just unwrap here.
        let _ = self.registration.deregister(io);
        self.registration =
            Registration::new_with_interest_and_handle(io, interest, scheduler::Handle::current())?;

        Ok(())
    }
}

feature! {
    #![any(feature = "net", all(unix, feature = "process"))]

    use crate::io::ReadBuf;
    use std::task::{Context, Poll};

    // MEMO: Eは TcpStreamの場合は mio::net::TcpStream, TcpListenerの場合は mio::net::TcpListener
    impl<E: Source> PollEvented<E> {
        // Safety: The caller must ensure that `E` can read into uninitialized memory
        pub(crate) unsafe fn poll_read<'a>(
            &'a self,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>>
        where
            &'a E: io::Read + 'a,
        {
            use std::io::Read;

            loop {
                let evt = ready!(self.registration.poll_read_ready(cx))?;

                let b = &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]);

                // used only when the cfgs below apply
                #[allow(unused_variables)]
                let len = b.len();

                match self.io.as_ref().unwrap().read(b) {
                    Ok(n) => {
                        // When mio is using the epoll or kqueue selector, reading a partially full
                        // buffer is sufficient to show that the socket buffer has been drained.
                        //
                        // This optimization does not work for level-triggered selectors such as
                        // windows or when poll is used.
                        //
                        // Read more:
                        // https://github.com/tokio-rs/tokio/issues/5866
                        #[cfg(all(
                            not(mio_unsupported_force_poll_poll),
                            any(
                                // epoll
                                target_os = "android",
                                target_os = "illumos",
                                target_os = "linux",
                                target_os = "redox",
                                // kqueue
                                target_os = "dragonfly",
                                target_os = "freebsd",
                                target_os = "ios",
                                target_os = "macos",
                                target_os = "netbsd",
                                target_os = "openbsd",
                                target_os = "tvos",
                                target_os = "visionos",
                                target_os = "watchos",
                            )
                        ))]
                        if 0 < n && n < len {
                            self.registration.clear_readiness(evt);
                        }

                        // Safety: We trust `TcpStream::read` to have filled up `n` bytes in the
                        // buffer.
                        buf.assume_init(n);
                        buf.advance(n);
                        return Poll::Ready(Ok(()));
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.registration.clear_readiness(evt);
                    }
                    Err(e) => return Poll::Ready(Err(e)),
                }
            }
        }

        pub(crate) fn poll_write<'a>(&'a self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>
        where
            &'a E: io::Write + 'a,
        {
            use std::io::Write;

            loop {
                let evt = ready!(self.registration.poll_write_ready(cx))?;

                match self.io.as_ref().unwrap().write(buf) {
                    Ok(n) => {
                        // if we write only part of our buffer, this is sufficient on unix to show
                        // that the socket buffer is full.  Unfortunately this assumption
                        // fails for level-triggered selectors (like on Windows or poll even for
                        // UNIX): https://github.com/tokio-rs/tokio/issues/5866
                        if n > 0 && (!cfg!(windows) && !cfg!(mio_unsupported_force_poll_poll) && n < buf.len()) {
                            self.registration.clear_readiness(evt);
                        }

                        return Poll::Ready(Ok(n));
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.registration.clear_readiness(evt);
                    }
                    Err(e) => return Poll::Ready(Err(e)),
                }
            }
        }

        #[cfg(any(feature = "net", feature = "process"))]
        pub(crate) fn poll_write_vectored<'a>(
            &'a self,
            cx: &mut Context<'_>,
            bufs: &[io::IoSlice<'_>],
        ) -> Poll<io::Result<usize>>
        where
            &'a E: io::Write + 'a,
        {
            use std::io::Write;
            self.registration.poll_write_io(cx, || self.io.as_ref().unwrap().write_vectored(bufs))
        }
    }
}

impl<E: Source> UnwindSafe for PollEvented<E> {}

impl<E: Source> RefUnwindSafe for PollEvented<E> {}

impl<E: Source> Deref for PollEvented<E> {
    type Target = E;

    fn deref(&self) -> &E {
        self.io.as_ref().unwrap()
    }
}

impl<E: Source + fmt::Debug> fmt::Debug for PollEvented<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollEvented").field("io", &self.io).finish()
    }
}

impl<E: Source> Drop for PollEvented<E> {
    fn drop(&mut self) {
        if let Some(mut io) = self.io.take() {
            // Ignore errors
            let _ = self.registration.deregister(&mut io);
        }
    }
}
