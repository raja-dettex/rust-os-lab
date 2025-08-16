
use std::io::stdout;

use crate::composite::GLOBAL_ALLOC;

#[test]
pub fn test_composite_allocator() {
    
    unsafe { GLOBAL_ALLOC.ensure_init() };
    println!("{}",GLOBAL_ALLOC.slab_block_size);
    println!("free bytes : {}" ,  GLOBAL_ALLOC.free_blocks()  );
    
    let boxed = Box::new(4 as u64);
    let b =*boxed;
    let alloc_size = core::mem::size_of::<u64>();
    println!("did alloc : {} and allocated {} bytes", *boxed, alloc_size);
    println!("free bytes : {}" , GLOBAL_ALLOC.free_blocks() );

    // next just do the vec alloc
    // let temp_vec = vec![0u8; 1024 * 1024];
    // //let temp_vec_size = core::mem::size_of::<vec![u8;10]>();
    // println!("free bytes : {}" , GLOBAL_ALLOC.free_blocks() );
    //println!("{}",GLOBAL_ALLOC.visualize_internal_fragmentation());


}