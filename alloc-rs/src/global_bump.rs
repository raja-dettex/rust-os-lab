use core::alloc::{Layout, GlobalAlloc};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr::NonNull;

use crate::bump::align_up;

const HEAP_SIZE: usize = 1024 * 1024;

static mut  GLOBAL_HEAP : [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];

pub struct GlobalBumpAllocator {
    start: AtomicUsize,
    end: AtomicUsize,
    next: AtomicUsize
}


impl GlobalBumpAllocator {
    pub const fn new_const() -> Self { 
        Self { 
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            next: AtomicUsize::new(0)
        }
    }

    pub fn ensure_init(&self,heap_addr: usize,end: usize) { 
        if self.start.load(Ordering::Acquire) != 0 { 
            return;
        }

        match self.start.compare_exchange(0, heap_addr, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => { 
                self.end.store(end, Ordering::Release);
                self.next.store(heap_addr, Ordering::Release);
            },
            Err(_) => {
                while self.end.load(Ordering::Acquire) == 0 {
                    core::hint::spin_loop();
                }
            },
        }
    }

    pub fn try_alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let heap_start = self.start.load(Ordering::Acquire);
        let heap_end = self.start.load(Ordering::Acquire);
        self.ensure_init(heap_start, heap_end);
        let align = layout.align();
        let size = layout.size();
        loop {
            let current = self.next.load(Ordering::Relaxed);
            let aligned = align_up(current, align);
            let new_next = aligned.saturating_add(size);
            match self.next.compare_exchange(current, new_next, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Some(unsafe {NonNull::new_unchecked(aligned as *mut u8)}),
                Err(_) =>  {
                    core::hint::spin_loop();
                    continue;
                },
            }
            
        }
    }

    pub fn reset(&self) {
        let start = self.start.load(Ordering::Acquire);
        self.next.store(start, Ordering::SeqCst);
    }

    pub fn free_bytes(&self) -> usize { 
        let end = self.end.load(Ordering::Acquire);
        let start = self.start.load(Ordering::Acquire);
        let next = self.next.load(Ordering::Acquire);
        if start == 0 || next >= end {
            0
        } else {
            return end - next;
        }
    }
}


//#[global_allocator]
pub static GLOBAL_BUMP_ALLOCATOR : GlobalBumpAllocator = GlobalBumpAllocator::new_const();

// unsafe impl GlobalAlloc for GlobalBumpAllocator {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         match self.try_alloc(layout) {
//             Some(ptr) => return ptr.as_ptr(),
//             None=> core::ptr::null_mut()
//         }
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         //
//     }
// }

