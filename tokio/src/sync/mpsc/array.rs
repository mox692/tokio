use std::marker::PhantomData;

#[derive(Debug)]
pub(crate) struct Rx<T> {
    _p: PhantomData<T>,
}
