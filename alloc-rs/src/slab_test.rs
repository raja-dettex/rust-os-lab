use crate::slab::Slab;


static mut HEAP: [u8; 1024] = [0u8; 1024];

//#[test]
pub fn simple_slab_test(){
    let heap_addr = unsafe { &raw mut HEAP as *mut u8 as usize};
    let heap_size = core::mem::size_of::<[u8; 1024]>();
    let mut slab_allocator = Slab::new_rounded(64);
    unsafe { slab_allocator.init_region(heap_addr, heap_size) } ;
    
    let a = unsafe { slab_allocator.alloc().expect("should alloc") };
    let b = unsafe { slab_allocator.alloc().expect("should alloc") };
    let c = unsafe { slab_allocator.alloc().expect("should alloc") };

    let ua = a.as_ptr() as  usize;
    let ub = b.as_ptr() as usize;
    println!("ptr a : {ua} and ptr b: {ub}");
    assert!(ua % 64 == 0, "unmatched blocs allocated");

    unsafe { slab_allocator.dealloc(b);}
    let d = unsafe { slab_allocator.alloc().expect("should alloc d")};
    assert!(d.as_ptr() as usize == ub , "address should be same");
}