use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize,Ordering};

// this is plain old bump allocator which contains only a start pointer and a tail pointer , allocs memory in contiguous blocks which is power of two, 
// does not reclaim free memory, to keep it simple only foward moving pointer is implemented,
// allocation is like fetch_add on next, 
// std is avoided and uses usize and core so that these can be compiled for no_std targets later

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize
}


impl BumpAllocator {
    pub fn new(heap_start: usize, heap_size : usize) -> Self {
        let heap_end = heap_start + heap_size;
        Self {
            heap_start,
            heap_end,
            next: AtomicUsize::new(heap_start)
        }
    }
    // all the allocs live the lifetime of the allocator, non_aliased for any other purpose.
    pub fn alloc(&self, layout:Layout) -> Option<NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();
        loop {
            let current = self.next.load(Ordering::Relaxed);
            let aligned = align_up(current,align);
            let new_next = aligned.saturating_add(size);
            if new_next > self.heap_end {
                return None;
            }
            match self.next.compare_exchange(current, new_next, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => return Some(unsafe {NonNull::new_unchecked(aligned as *mut u8)}),
                Err(_) => {
                    core::hint::spin_loop();
                    continue;
                },
            }
        }

    }
    // reset is not for concurrent alloc, it is not thread safe.
    pub fn reset(&self) { 
        self.next.store(self.heap_start, Ordering::Relaxed);
    }

    pub fn free_bytes(&self)-> usize  { 
        let current = self.next.load(Ordering::Relaxed);
        if current >= self.heap_end {
            0
        } else {
            self.heap_end  - current
        }
    }
}

pub fn align_up(addr: usize, align: usize) ->  usize {
    if align == 0 {
        addr
    } else {
        (addr + align - 1) & !(align - 1)
    }
}