use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::alloc::{GlobalAlloc, Layout, System};

struct TrackedAlloc {}

#[global_allocator]
static ALLOC: TrackedAlloc = TrackedAlloc {};

static TOTAL_MEM: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackedAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            TOTAL_MEM.fetch_add(layout.size(), Relaxed);
        }
        ret
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        TOTAL_MEM.fetch_sub(layout.size(), Relaxed);
        System.dealloc(ptr, layout);
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc_zeroed(layout);
        if !ret.is_null() {
            TOTAL_MEM.fetch_add(layout.size(), Relaxed);
        }
        ret
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ret = System.realloc(ptr, layout, new_size);
        if !ret.is_null() {
            TOTAL_MEM.fetch_add(new_size.wrapping_sub(layout.size()), Relaxed);
        }
        ret
    }
}

#[tokio::main]
async fn main() {
    let s = 3;

    let p = TOTAL_MEM.load(Relaxed);
    println!("val {p}")
}
