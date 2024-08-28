use std::{
    alloc::{alloc, alloc_zeroed, dealloc, Layout},
    io::Write,
};

#[derive(Debug)]
pub struct Memory {
    pub inner: *mut u8, // Vec<u8>,
    layout: Layout,     // TODO: Remove this
    write_pos: usize,
    size: usize,
}

/// Types must implement this to work with `Memory`
///
/// ### !! Warning !!
///
/// Non-primitive types who implements this trait
/// can cause the program to crash, because of malformation of the type.
///
/// In implemetation below the program will crash because of the is no way tho construct `Foo` from `1u8`.
/// ```
/// #[repr(u8)]
/// enum Foo {
///     Bar = 0u8,
/// }
/// unsafe impl MemoryAllowed for Foo {}
/// let mut mem = Memory::new(16);
/// mem.write(0, 1u8);
/// mem.read::<Foo>(0);
/// ```
///
pub unsafe trait MemoryAllowed {}
unsafe impl MemoryAllowed for u8 {}
unsafe impl MemoryAllowed for u16 {}
unsafe impl MemoryAllowed for u32 {}
unsafe impl MemoryAllowed for u64 {}
unsafe impl MemoryAllowed for i8 {}
unsafe impl MemoryAllowed for i16 {}
unsafe impl MemoryAllowed for i32 {}
unsafe impl MemoryAllowed for i64 {}

const MEM_MIN: usize = 16;

impl Drop for Memory {
    fn drop(&mut self) {
        unsafe { dealloc(self.inner, self.layout) };
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new(MEM_MIN)
    }
}

impl Memory {
    pub fn new(size: usize) -> Self {
        let size = if size > MEM_MIN { size } else { MEM_MIN };
        let layout = Layout::from_size_align(size, size_of::<u8>()).unwrap();
        Self {
            inner: unsafe { alloc_zeroed(layout) }, //vec![0; size],
            layout,
            write_pos: 0,
            size,
        }
    }

    pub fn realloc(&mut self, new_size: usize) {
        assert!(new_size > self.size, "Cannot realloc less memory");
        unsafe {
            let new_layout = Layout::from_size_align(new_size, size_of::<u8>()).unwrap();
            let new_inner = alloc(new_layout);
            new_inner.copy_from(self.inner, self.size);
            dealloc(self.inner, self.layout);
            self.inner = new_inner;
            self.layout = new_layout;
            self.size = new_size;
        }
    }

    pub fn write_push<T: Copy + MemoryAllowed>(&mut self, value: T) {
        let size_of = size_of::<T>();
        if self.write_pos + size_of >= self.size {
            self.realloc(self.size + size_of);
        }
        self.write(self.write_pos, value);
        self.write_pos += size_of;
    }

    pub fn read_push<T: Copy + MemoryAllowed>(&mut self) -> T {
        let size_of = size_of::<T>();
        assert!(self.write_pos + size_of <= self.size);
        let res = self.read::<T>(self.write_pos);
        self.write_pos += size_of;

        res
    }

    pub fn read<T: Copy + MemoryAllowed>(&self, index: usize) -> T {
        let size_of = size_of::<T>();
        let byte_index = index;
        assert!(byte_index + size_of <= self.size, "Index out of bounds");
        unsafe { (self.inner.add(index) as *const T).read_unaligned() }
    }

    pub fn write<T: Copy + MemoryAllowed>(&mut self, index: usize, value: T) {
        let size_of = size_of::<T>();
        let byte_index = index;
        assert!(byte_index + size_of <= self.size, "Index out of bounds");
        unsafe {
            (self.inner.add(index) as *mut T).write_unaligned(value);
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        unsafe {
            if let Some(a) = std::ptr::slice_from_raw_parts(self.inner, self.size).as_ref() {
                let _ = buf.write(a);
            }
        }
        buf
    }

    pub fn to_ptr<T: Copy + MemoryAllowed>(&self) -> *mut T {
        unsafe {
            return self.inner.add(self.write_pos) as *mut T;
        }
    }

    pub fn set_write_pos(&mut self, pos: usize) {
        self.write_pos = pos;
    }

    pub fn get_write_pos(&self) -> usize {
        self.write_pos
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
