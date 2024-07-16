#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_malloc(size: usize) -> *mut std::ffi::c_void {
    unsafe { libmimalloc_sys::mi_malloc(size) }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_free(p: *mut std::ffi::c_void) {
    unsafe {
        libmimalloc_sys::mi_free(p);
    }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_realloc(p: *mut std::ffi::c_void, newsize: usize) -> *mut std::ffi::c_void {
    unsafe { libmimalloc_sys::mi_realloc(p, newsize) }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_usable_size(p: *const std::ffi::c_void) -> usize {
    unsafe { libmimalloc_sys::mi_usable_size(p) }
}
