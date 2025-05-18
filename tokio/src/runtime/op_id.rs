// TODO: Put together with other id related utils.

use std::num::NonZeroU64;

#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub(crate) struct OpId(NonZeroU64);

impl OpId {
    pub(crate) fn next() -> Self {
        use crate::loom::sync::atomic::Ordering::Relaxed;
        use crate::loom::sync::atomic::StaticAtomicU64;

        #[cfg(all(test, loom))]
        crate::loom::lazy_static! {
            static ref NEXT_ID: StaticAtomicU64 = StaticAtomicU64::new(1);
        }

        #[cfg(not(all(test, loom)))]
        static NEXT_ID: StaticAtomicU64 = StaticAtomicU64::new(1);

        loop {
            let id = NEXT_ID.fetch_add(1, Relaxed);
            if let Some(id) = NonZeroU64::new(id) {
                return Self(id);
            }
        }
    }

    pub(crate) fn as_u64(&self) -> u64 {
        self.0.get()
    }
}
