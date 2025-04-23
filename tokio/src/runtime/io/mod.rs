//! This module can be accessed from either
//! * `tokio_unstable_uring`
//! * `cfg_io_driver`

#![cfg_attr(not(all(feature = "rt", feature = "net")), allow(dead_code))]

cfg_io_driver_impl! {
    mod driver;
    use driver::{Direction, Tick};
    pub(crate) use driver::{Driver, Handle, ReadyEvent};

    mod registration;
    pub(crate) use registration::Registration;

    mod registration_set;
    use registration_set::RegistrationSet;

    mod scheduled_io;
    use scheduled_io::ScheduledIo;

    mod metrics;
    use metrics::IoDriverMetrics;

    use crate::util::ptr_expose::PtrExposeDomain;
    static EXPOSE_IO: PtrExposeDomain<ScheduledIo> = PtrExposeDomain::new();
}

// `tokio_unstable_uring`
cfg_not_io_driver! {
    cfg_tokio_unstable_uring! {
        mod uring;

        use uring::driver::{Direction, Tick};

        use uring::registration_set;

        pub(crate) use uring::registration_set::RegistrationSet;
        pub(crate) use uring::driver::{Driver, Handle, ReadyEvent};

        use uring::scheduled_io::ScheduledIo;
        use uring::metrics::IoDriverMetrics;

        use crate::util::ptr_expose::PtrExposeDomain;
        static EXPOSE_IO: PtrExposeDomain<ScheduledIo> = PtrExposeDomain::new();
    }
}
