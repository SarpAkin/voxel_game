use std::{slice, mem::ManuallyDrop};

pub fn boxed_slice_to_array<T,const N: usize>(bslice:Box<[T]>) -> Option<Box<[T;N]>>{
    if N != bslice.len() {return None;}

    unsafe {
        Some(Box::from_raw(Box::into_raw(bslice) as *mut [T;N]))
    }
}
