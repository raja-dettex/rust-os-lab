use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering,AtomicPtr};
use core::ptr::NonNull;
use std::io::Cursor;
use std::vec;

use crate::bump::align_up;

// sinlgly-linked free struct node
#[repr(C)]
struct FreeNode { 
    next : usize // the raw pointer to the next node
}


pub type StrippedLayout = (usize, usize);

pub struct Slab { 
    pub block_size: usize,
    pub head: AtomicUsize,
    pub region_start : usize,
    pub region_end : usize,
    pub blocks : UnsafeCell<Vec<StrippedLayout>>
}

// fixed size lock free slab
// caller has to provide and gurantee the memory region
// free blocks are linked in atomic lifo stack of head
// alloc pops from the stack and dealloc pushed to the stack again
// exactly call once the init_region() before using this slab
// the block is size is minimum core::mem::<usize>() or multiple of this
// new_rounded() ensures this

impl Slab { 

    pub const fn new_rounded(block_size : usize)-> Self  { 
        let min_size = core::mem::size_of::<usize>();
        let size = if block_size > min_size {block_size} else {min_size};
        let align = core::mem::size_of::<usize>();
        let block_size = align_slab_up(size, align);
        Self {
            block_size,
            head: AtomicUsize::new(0),
            region_start: 0,
            region_end:0,
            blocks: UnsafeCell::new(Vec::new())
        }
    }

    // caller has to gurantee that region is valid and exclusively owned by the slab
    // for its entire lifetime.
    pub unsafe fn init_region(&mut self, start: usize, size: usize)  {
        let end  = start.saturating_add(size);
        let region_start  = align_up(start, core::mem::size_of::<usize>());
        let region_end = end;
        self.region_start = region_start;
        self.region_end = region_end;

        // building a lifo free list by walking the region of block_size steps
        let mut cursor = region_start;
        let mut head = 0 as usize;
        while cursor.saturating_add(self.block_size) <= region_end {
            let node = cursor as *mut FreeNode;
             (*node).next = head; 
            head = cursor;
            cursor = cursor.saturating_add(self.block_size);
        }
        self.head.store(head, Ordering::SeqCst);
    }

    pub unsafe fn alloc(&self) -> Option<NonNull<u8>> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            if head == 0 {
                return None;
            }
            let next = unsafe { (head as *const FreeNode).read().next};
            match self.head.compare_exchange(head, next, Ordering::AcqRel, Ordering::Acquire) { 
                Ok(_) => return Some(unsafe { NonNull::new_unchecked(head as *mut u8) } ),
                Err(_) => {
                    core::hint::spin_loop();
                }
            }
        }
    }

    pub unsafe fn dealloc(&self, ptr: NonNull<u8>) {
        let p = ptr.as_ptr() as usize;
        debug_assert!(self.owns(p));
        loop {
            let head = self.head.load(Ordering::Acquire);
            let node = unsafe { p as *mut FreeNode };
            (*node).next = head;
            match self.head.compare_exchange(head, p, Ordering::AcqRel, Ordering::Acquire) { 
                Ok(_) => return,
                Err(_) => {
                    core::hint::spin_loop();
                }
            }
        }
    }

    #[inline]
    pub fn owns(&self, p: usize) -> bool {
        return p >= self.region_start && p <= self.region_end;
    }

    pub fn block_size(&self) -> usize { 
        return self.block_size;
    }

    pub fn debug_count_free(&self) -> usize { 
        let mut count = 0;
        let mut head = self.head.load(Ordering::Acquire);
        while head != 0 {
            head = unsafe { (head as *const FreeNode).read().next};
            count += 1;
        }
        return count;
    }
}


pub const fn align_slab_up(addr: usize, align: usize) ->  usize {
    if align == 0 {
        addr
    } else {
        (addr + align - 1) & !(align - 1)
    }
}