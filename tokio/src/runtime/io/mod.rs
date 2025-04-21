#![allow(dead_code, unused_variables, unused_imports)]
#![cfg_attr(not(all(feature = "rt", feature = "net")), allow(dead_code))]
mod driver;
pub(crate) use driver::{Driver, Handle, ReadyEvent};

cfg_io_driver_impl! {
    use driver::{Direction, Tick};
    mod registration;
    pub(crate) use registration::Registration;
    mod registration_set;
    use registration_set::RegistrationSet;

    mod scheduled_io;
    use scheduled_io::ScheduledIo;

    mod metrics;
    use metrics::IoDriverMetrics;
    static EXPOSE_IO: PtrExposeDomain<ScheduledIo> = PtrExposeDomain::new();
    use crate::util::ptr_expose::PtrExposeDomain;
}

cfg_not_io_driver! {
    use crate::io::Ready;
    use crate::io::Interest;
    use crate::loom::sync::Arc;
    use crate::runtime::io::driver::Tick;
    use std::task::Context;
    use std::task::Poll;
    use std::marker::PhantomData;
    use crate::runtime::io::driver::Direction;
    use crate::util::ptr_expose::PtrExposeDomain;

    static EXPOSE_IO: PtrExposeDomain<ScheduledIo> = PtrExposeDomain::new();


    pub(crate) mod registration_set {
        pub(crate) struct Synced{}
    }

    use registration_set::Synced;

    struct RegistrationSet {}

    impl RegistrationSet {
        pub(super) fn new() -> (RegistrationSet, Synced) {
            todo!()
        }

        pub(super) fn is_shutdown(&self, synced: &Synced) -> bool {
            todo!()
        }

        /// Returns `true` if there are registrations that need to be released
        pub(super) fn needs_release(&self) -> bool {
            todo!()
        }

        pub(super) fn allocate(&self, synced: &mut Synced) -> std::io::Result<Arc<ScheduledIo>> {
            todo!()
        }

        // Returns `true` if the caller should unblock the I/O driver to purge
        // registrations pending release.
        pub(super) fn deregister(&self, synced: &mut Synced, registration: &Arc<ScheduledIo>) -> bool {
            todo!()
        }

        pub(super) fn shutdown(&self, synced: &mut Synced) -> Vec<Arc<ScheduledIo>> {
            todo!()
        }

        pub(super) fn release(&self, synced: &mut Synced) {
            todo!()
        }

        // This function is marked as unsafe, because the caller must make sure that
        // `io` is part of the registration set.
        pub(super) unsafe fn remove(&self, synced: &mut Synced, io: &Arc<ScheduledIo>) {
            todo!()
        }
    }
    pub(crate) struct ScheduledIo {}
    impl ScheduledIo {
        pub(crate) fn token(&self) -> mio::Token {
            todo!()
        }
        pub(super) fn shutdown(&self) {
            todo!()
        }
        fn set_readiness(&self, tick_op: Tick, f: impl Fn(Ready) -> Ready) {
            todo!()
        }
        pub(super) fn wake(&self, ready: Ready) {
            todo!()
        }

        pub(super) fn ready_event(&self, interest: Interest) -> ReadyEvent {
            todo!()
        }
        fn poll_readiness(
            &self,
            cx: &mut Context<'_>,
            direction: Direction,
        ) -> Poll<ReadyEvent> {
            todo!()
        }
        pub(crate) fn clear_readiness(&self, event: ReadyEvent) {
            todo!()
        }
        pub(crate) fn clear_wakers(&self) {
            todo!()
        }
        pub(crate) async fn readiness(&self, interest: Interest) -> ReadyEvent {
            todo!()
        }
        fn readiness_fut(&self, interest: Interest) -> Readiness<'_> {
            todo!()
        }
    }
    struct Readiness<'a>(PhantomData<&'a ()>);


    #[derive(Default)]
    pub(crate) struct IoDriverMetrics{}
    impl IoDriverMetrics {
        pub(crate) fn incr_fd_count(&self) {}
        pub(crate) fn dec_fd_count(&self) {}
        pub(crate) fn incr_ready_count_by(&self, _amt: u64) {}
    }

}
