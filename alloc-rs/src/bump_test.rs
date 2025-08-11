
use std::alloc::Layout;
use std::io::stdout;
use std::ptr;
use crate::bump::BumpAllocator;

static mut TEST_HEAP :[u8;1024] = [0u8; 1024];

#[test]
pub fn simple_alloc_and_test() {
    let heap_addr = unsafe { &raw mut TEST_HEAP as *mut _ as usize};
    let heap_size = core::mem::size_of::<[u8;1024]>();
    println!("heap addr : {heap_addr} and heap_size : {heap_size}");
    let allocator = BumpAllocator::new(heap_addr, heap_size);
    let l1 = Layout::from_size_align(16, 8).unwrap();
    println!("l1: {l1:?}");
    let ptr1 = allocator.alloc(l1).expect("expected to alloc l1");
    let ptr2 = allocator.alloc(l1).expect("expected to alloc l1");
    println!("ptr 1 : {ptr1:?} and ptr2: {ptr2:?}");
    let p1 = ptr1.as_ptr() as usize;
    let p2 = ptr2.as_ptr() as usize;
    println!("{p1:?} {p2}");
    assert_eq!(p2 - p1,16);
    println!("{heap_addr}");
    let big = Layout::from_size_align(2000, 8).unwrap();
    assert!(allocator.alloc(big).is_none());
    allocator.reset();
    allocator.alloc(l1).expect("this alloc will succeed");

}

#[test]
pub fn alloc_alignment_test() {
    let heap_addr = unsafe { &raw mut TEST_HEAP as *mut _ as usize};
    let heap_size = 1024 as usize;
    let allocator = BumpAllocator::new(heap_addr, heap_size);
    let l1 = Layout::from_size_align(1, 1).unwrap();
    let ptr = allocator.alloc(l1).expect("expected to succeed");
    assert!((ptr.as_ptr() as usize) % 1 == 0, "alignment is mismatched");
}

use crate::global_bump::GLOBAL_BUMP_ALLOCATOR as GLOBAL;

#[test]
pub fn simple_global_alloc() {
    println!("runing test");
    GLOBAL.reset();
    println!("reset");

    let b = Box::new(42u64);
    println!("did alloc");
    println!("{}", *b);
    let mut vec_alloc = Vec::with_capacity(10);
    for i in 0..10 {
        vec_alloc.push(i as u16);
    }
    let free_bytes = GLOBAL.free_bytes();
    println!("after box alloc free bytes are {free_bytes}");
}