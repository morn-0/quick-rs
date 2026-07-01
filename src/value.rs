use crate::{context::Context, error::Error};
use quickjs_sys as sys;
use std::{
    ffi::c_void,
    mem::{self, ManuallyDrop},
    ptr, slice,
};

extern "C" {
    fn JS_VALUE_GET_TAG_real(v: sys::JSValue) -> i32;
    fn JS_VALUE_GET_INT_real(v: sys::JSValue) -> i32;
    fn JS_VALUE_GET_FLOAT64_real(v: sys::JSValue) -> f64;
    fn JS_VALUE_GET_PTR_real(v: sys::JSValue) -> *mut c_void;
    fn JS_DupValue_real(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue;
    fn JS_FreeValue_real(ctx: *mut sys::JSContext, v: sys::JSValue);
    fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_MKPTR_real(tag: i32, ptr: *mut c_void) -> sys::JSValue;
    fn JS_NewFloat64_real(ctx: *mut sys::JSContext, val: f64) -> sys::JSValue;
}

#[inline]
pub(crate) fn tag_of(v: sys::JSValue) -> i32 {
    unsafe { JS_VALUE_GET_TAG_real(v) }
}

#[inline]
pub(crate) fn ptr_of(v: sys::JSValue) -> *mut c_void {
    unsafe { JS_VALUE_GET_PTR_real(v) }
}

#[inline]
pub(crate) fn dup(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue {
    unsafe { JS_DupValue_real(ctx, v) }
}

#[inline]
pub(crate) fn free(ctx: *mut sys::JSContext, v: sys::JSValue) {
    unsafe { JS_FreeValue_real(ctx, v) }
}

#[inline]
pub(crate) fn mkval(tag: i32, val: i32) -> sys::JSValue {
    unsafe { JS_MKVAL_real(tag, val) }
}

#[inline]
pub(crate) fn mkptr(tag: i32, ptr: *mut c_void) -> sys::JSValue {
    unsafe { JS_MKPTR_real(tag, ptr) }
}

#[inline]
pub(crate) fn new_float64(ctx: *mut sys::JSContext, d: f64) -> sys::JSValue {
    unsafe { JS_NewFloat64_real(ctx, d) }
}

#[inline]
pub(crate) fn new_atom(ctx: *mut sys::JSContext, name: &str) -> sys::JSAtom {
    unsafe { sys::JS_NewAtomLen(ctx, name.as_ptr().cast(), name.len()) }
}

#[inline]
pub(crate) fn free_atom(ctx: *mut sys::JSContext, atom: sys::JSAtom) {
    unsafe { sys::JS_FreeAtom(ctx, atom) }
}

pub trait Number: Sized {
    fn from_f64(value: f64) -> Self;
}

macro_rules! impl_number {
    ($($ty:ty),+ $(,)?) => {
        $(impl Number for $ty {
            fn from_f64(value: f64) -> Self {
                value as Self
            }
        })+
    };
}

impl_number!(i8, u8, i16, u16, i32, u32, f32, f64);

pub struct Value {
    pub(crate) ctx: Context,
    pub(crate) raw: sys::JSValue,
}

impl Value {
    pub(crate) fn from_raw(ctx: Context, raw: sys::JSValue) -> Self {
        Value { ctx, raw }
    }

    pub(crate) fn into_raw(self) -> sys::JSValue {
        let mut this = ManuallyDrop::new(self);
        let raw = this.raw;
        unsafe { ptr::drop_in_place(&mut this.ctx) };
        raw
    }

    pub fn raw(&self) -> sys::JSValue {
        self.raw
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn borrow(&self) -> ValueRef<'_> {
        ValueRef {
            ctx: &self.ctx,
            raw: self.raw,
        }
    }

    pub fn tag(&self) -> i32 {
        tag_of(self.raw)
    }

    pub fn to_i32(&self) -> Result<i32, Error> {
        self.borrow().to_i32()
    }

    pub fn to_f64(&self) -> Result<f64, Error> {
        self.borrow().to_f64()
    }

    pub fn to_number<T: Number>(&self) -> Result<T, Error> {
        self.borrow().to_number()
    }

    pub fn to_bool(&self) -> Result<bool, Error> {
        self.borrow().to_bool()
    }

    pub fn to_string(&self) -> Result<String, Error> {
        self.borrow().to_string()
    }

    pub fn to_json(&self) -> Result<String, Error> {
        self.borrow().to_json()
    }

    pub fn to_array(&self) -> Result<Vec<Value>, Error> {
        self.borrow().to_array()
    }

    pub fn get_property(&self, key: impl AsRef<str>) -> Result<Value, Error> {
        self.borrow().get_property(key)
    }

    pub fn set_property(&self, key: impl AsRef<str>, value: Value) -> Result<(), Error> {
        self.borrow().set_property(key, value)
    }

    /// 可变访问底层 ArrayBuffer。
    ///
    /// # Safety
    /// 调用者须保证此期间不存在任何其它读写同一缓冲区的存活别名（包括本 `Value` 的克隆）。
    pub unsafe fn to_buffer_mut<T: Number>(&mut self) -> Result<&mut [T], Error> {
        let (ptr, size) = self.array_buffer()?;
        Ok(unsafe { slice::from_raw_parts_mut(ptr.cast::<T>(), size / mem::size_of::<T>()) })
    }

    pub fn to_buffer<T: Number>(&self) -> Result<&[T], Error> {
        let (ptr, size) = self.array_buffer()?;
        Ok(unsafe { slice::from_raw_parts(ptr.cast::<T>(), size / mem::size_of::<T>()) })
    }

    fn array_buffer(&self) -> Result<(*mut u8, usize), Error> {
        if !unsafe { sys::JS_IsArrayBuffer(self.raw) } {
            return Err(Error::Type {
                expected: "ArrayBuffer",
                got: self.tag(),
            });
        }

        let mut size = 0usize;
        let ptr = unsafe { sys::JS_GetArrayBuffer(self.ctx.as_raw(), &mut size, self.raw) };
        if ptr.is_null() {
            return Err(Error::Null("JS_GetArrayBuffer"));
        }

        Ok((ptr, size))
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        Value {
            ctx: self.ctx.clone(),
            raw: dup(self.ctx.as_raw(), self.raw),
        }
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        free(self.ctx.as_raw(), self.raw);
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.borrow(), f)
    }
}

#[derive(Clone, Copy)]
pub struct ValueRef<'a> {
    pub(crate) ctx: &'a Context,
    pub(crate) raw: sys::JSValue,
}

impl<'a> ValueRef<'a> {
    pub fn context(&self) -> &'a Context {
        self.ctx
    }

    pub fn raw(&self) -> sys::JSValue {
        self.raw
    }

    pub fn tag(&self) -> i32 {
        tag_of(self.raw)
    }

    pub fn to_owned(&self) -> Value {
        Value {
            ctx: self.ctx.clone(),
            raw: dup(self.ctx.as_raw(), self.raw),
        }
    }

    pub fn to_i32(&self) -> Result<i32, Error> {
        if self.tag() == sys::JS_TAG_INT {
            Ok(unsafe { JS_VALUE_GET_INT_real(self.raw) })
        } else {
            Err(Error::Type {
                expected: "int",
                got: self.tag(),
            })
        }
    }

    pub fn to_f64(&self) -> Result<f64, Error> {
        if self.tag() == sys::JS_TAG_FLOAT64 {
            Ok(unsafe { JS_VALUE_GET_FLOAT64_real(self.raw) })
        } else {
            Err(Error::Type {
                expected: "float64",
                got: self.tag(),
            })
        }
    }

    pub fn to_number<T: Number>(&self) -> Result<T, Error> {
        let mut out = 0.0;
        let ret = unsafe { sys::JS_ToFloat64(self.ctx.as_raw(), &mut out, self.raw) };
        if ret < 0 {
            Err(Error::Exception(self.ctx.catch()))
        } else {
            Ok(T::from_f64(out))
        }
    }

    pub fn to_bool(&self) -> Result<bool, Error> {
        if self.tag() == sys::JS_TAG_BOOL {
            Ok(unsafe { JS_VALUE_GET_INT_real(self.raw) } != 0)
        } else {
            Err(Error::Type {
                expected: "bool",
                got: self.tag(),
            })
        }
    }

    pub fn to_string(&self) -> Result<String, Error> {
        let ctx = self.ctx.as_raw();

        let mut len = 0usize;
        let data = unsafe { sys::JS_ToCStringLen2(ctx, &mut len, self.raw, false) };
        if data.is_null() {
            return Err(Error::Exception(self.ctx.catch()));
        }

        let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), len) };
        let text = String::from_utf8_lossy(bytes).into_owned();

        unsafe { sys::JS_FreeCString(ctx, data) };
        Ok(text)
    }

    pub fn to_json(&self) -> Result<String, Error> {
        let ctx = self.ctx.as_raw();

        let undefined = mkval(sys::JS_TAG_UNDEFINED, 0);
        let raw = unsafe { sys::JS_JSONStringify(ctx, self.raw, undefined, undefined) };

        let value = self.ctx.check(Value::from_raw(self.ctx.clone(), raw))?;
        value.to_string()
    }

    pub fn to_array(&self) -> Result<Vec<Value>, Error> {
        let ctx = self.ctx.as_raw();

        let len = self.get_property("length")?.to_i32()?;
        let mut out = Vec::with_capacity(len.max(0) as usize);

        for i in 0..len {
            let raw = unsafe { sys::JS_GetPropertyUint32(ctx, self.raw, i as u32) };
            out.push(self.ctx.check(Value::from_raw(self.ctx.clone(), raw))?);
        }

        Ok(out)
    }

    pub fn get_property(&self, key: impl AsRef<str>) -> Result<Value, Error> {
        let ctx = self.ctx.as_raw();

        let atom = new_atom(ctx, key.as_ref());
        let raw = unsafe { sys::JS_GetProperty(ctx, self.raw, atom) };
        free_atom(ctx, atom);

        self.ctx.check(Value::from_raw(self.ctx.clone(), raw))
    }

    pub fn set_property(&self, key: impl AsRef<str>, value: Value) -> Result<(), Error> {
        let ctx = self.ctx.as_raw();

        let atom = new_atom(ctx, key.as_ref());
        let ret = unsafe { sys::JS_SetProperty(ctx, self.raw, atom, value.into_raw()) };
        free_atom(ctx, atom);

        if ret < 0 {
            Err(Error::Exception(self.ctx.catch()))
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_property_raw(&self, key: &str) -> Value {
        let ctx = self.ctx.as_raw();

        let atom = new_atom(ctx, key);
        let raw = unsafe { sys::JS_GetProperty(ctx, self.raw, atom) };
        free_atom(ctx, atom);

        Value::from_raw(self.ctx.clone(), raw)
    }
}

impl std::fmt::Debug for ValueRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_json() {
            Ok(json) => write!(f, "Value({json})"),
            Err(_) => write!(f, "Value(tag {})", self.tag()),
        }
    }
}
