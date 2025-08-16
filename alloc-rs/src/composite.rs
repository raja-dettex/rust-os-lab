use core::{alloc::GlobalAlloc, cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull, sync::atomic::{AtomicUsize, Ordering}};
use std::rc::Rc;

use crate::{bump::align_up, global_bump::GlobalBumpAllocator, slab::{Slab, StrippedLayout}};


pub struct CompositeAllocator { 
    pub inited: AtomicUsize,
    pub bump_allocator: GlobalBumpAllocator,
    pub slab_allocator : UnsafeCell<MaybeUninit<Slab>>,
    pub slab_block_size : usize,

}

const HEAP_SIZE : usize = 1024 *1024;
const SLOTS : [AtomicUsize;10] = {
    const fn make_arr() -> [AtomicUsize; 10] {
        let mut arr: [MaybeUninit<AtomicUsize>; 10] = unsafe {
            MaybeUninit::<[MaybeUninit<AtomicUsize>; 10]>::uninit().assume_init()
        };
        let mut i = 0;
        while i < 10 {
            arr[i] = MaybeUninit::new(AtomicUsize::new(0));
            i += 1;
        }
        unsafe { core::mem::transmute(arr)}
    }
    make_arr()
};

static mut GLOBAL_HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
const SLAB_REGION_BYTES: usize = 128 * 1024;
const SLAB_WORD_ALIGN: usize = core::mem::size_of::<usize>();
impl  CompositeAllocator { 
    pub const fn new_const(slab_block_size : usize) -> Self { 
        Self{ 
            inited: AtomicUsize::new(0),
            bump_allocator: GlobalBumpAllocator::new_const(),
            slab_allocator: UnsafeCell::new(MaybeUninit::new(Slab::new_rounded(slab_block_size))),
            slab_block_size

        }
        
    }

    // pub fn visualize_internal_fragmentation(&self) -> String { 
    //     let mut str = String::new();
    //     let idx = self.blocks_touched_idx.load(Ordering::Acquire);
    //     let _ = self.slots_occupancy[..idx].iter().map(|val| {
    //         let total_val = val.load(Ordering::Acquire);
    //         let total_free = self.slots_free[idx].load(Ordering::Acquire);
    //         for i in 0..total_val { 
    //             str.push('#');
    //         }
    //         for i in 0..total_free { 
    //             str.push('.');
    //         }
    //     });
    //     str
    // }

    pub fn ensure_init(&self) {
        let current = self.inited.load(Ordering::Acquire); 
        if current == 2 {
            return
        }
        if self.inited.compare_exchange(current, 2, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            let heap_addr = unsafe { &raw mut GLOBAL_HEAP as *mut u8 as usize};
            let heap_end = heap_addr + HEAP_SIZE;
            let slab_size = SLAB_REGION_BYTES.min(heap_end - heap_addr);
            let slab_start = align_up(heap_addr, SLAB_WORD_ALIGN);
            let slab_end = slab_start + slab_size;
         
            // init the bump
            let bump_start = align_up(slab_end, core::mem::size_of::<usize>());
            self.bump_allocator.ensure_init(bump_start, heap_end);
            unsafe { (&mut *(*self.slab_allocator.get()).as_mut_ptr()).init_region(slab_start, slab_end - slab_start);};
            self.inited.store(2, Ordering::SeqCst);
        } else { 
            while self.inited.load(Ordering::Acquire) !=2 { 
                core::hint::spin_loop();
            }
        }
    }

    pub fn free_blocks(&self) -> usize { 
        let slab = unsafe { (&*(*self.slab_allocator.get()).as_ptr())};
        let slab_free = slab.debug_count_free();
        slab_free
    }
}


#[global_allocator]
pub static GLOBAL_ALLOC: CompositeAllocator = CompositeAllocator::new_const(64);

unsafe impl Sync for CompositeAllocator {}

unsafe impl GlobalAlloc for CompositeAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.ensure_init();
        let size = layout.size();
        let align = layout.align();

        if size <= self.slab_block_size && align <= SLAB_WORD_ALIGN {
            let slab = unsafe { (&*(*self.slab_allocator.get()).as_ptr())};
            if let Some(p) = slab.alloc() {
                return p.as_ptr();
            } else {
                return core::ptr::null_mut();
            }
        } else {
            match self.bump_allocator.try_alloc(layout) {
                Some(p) => return p.as_ptr(),
                None => return core::ptr::null_mut(),
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        if self.inited.load(Ordering::Acquire) != 2 {
            return;
        }

        let p = ptr as usize;
        let slab = unsafe { &*(*self.slab_allocator.get()).as_ptr()};
        if layout.size() <= self.slab_block_size && layout.align() <= SLAB_WORD_ALIGN &&  slab.owns(p) {
            slab.dealloc(unsafe {NonNull::new_unchecked(ptr)});
        }
    }
}