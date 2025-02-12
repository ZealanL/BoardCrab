use std::marker::PhantomData;

#[derive(Debug, Copy, Clone)]
pub struct RawPtr<T> {
    phantom: PhantomData<T>,
    ptr_val: usize
}

impl<T> RawPtr<T> {
    pub fn new(val_ref: &mut T) -> RawPtr<T> {
        unsafe {
            let ptr = val_ref as *mut T;
            let ptr_val = ptr as usize;
            RawPtr { phantom: PhantomData::<T>, ptr_val }
        }
    }

    pub fn get(&self) -> &mut T {
        unsafe {
            let ptr = self.ptr_val as *mut T;
            &mut *ptr
        }
    }
}