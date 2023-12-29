use crate::io::{AsyncRead, ReadBuf};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

cfg_io_util! {
    /// An async reader which yields one byte over and over and over and over and
    /// over and...
    ///
    /// This struct is generally created by calling [`repeat`][repeat]. Please
    /// see the documentation of `repeat()` for more details.
    ///
    /// This is an asynchronous version of [`std::io::Repeat`][std].
    ///
    /// [repeat]: fn@repeat
    /// [std]: std::io::Repeat
    #[derive(Debug)]
    pub struct Repeat {
        byte: u8,
    }

    /// Creates an instance of an async reader that infinitely repeats one byte.
    ///
    /// All reads from this reader will succeed by filling the specified buffer with
    /// the given byte.
    ///
    /// This is an asynchronous version of [`std::io::repeat`][std].
    ///
    /// [std]: std::io::repeat
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::{self, AsyncReadExt};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut buffer = [0; 3];
    ///     io::repeat(0b101).read_exact(&mut buffer).await.unwrap();
    ///     assert_eq!(buffer, [0b101, 0b101, 0b101]);
    /// }
    /// ```
    pub fn repeat(byte: u8) -> Repeat {
        Repeat { byte }
    }
}

impl AsyncRead for Repeat {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        // TODO: could be faster, but should we unsafe it?
        while buf.remaining() != 0 {
            buf.put_slice(&[self.byte]);
        }
        Poll::Ready(Ok(()))
    }
}

cfg_coop! {
    fn poll_proceed_and_make_progress(cx: &mut Context<'_>) -> Poll<()> {
        let coop = ready!(crate::runtime::coop::poll_proceed(cx));
        coop.made_progress();
        Poll::Ready(())
    }
}

cfg_not_coop! {
    fn poll_proceed_and_make_progress(_: &mut Context<'_>) -> Poll<()> {
        Poll::Ready(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        crate::is_unpin::<Repeat>();
    }
}
