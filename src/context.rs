use crate::{
    error::{Error, Exception},
    runtime::Runtime,
    value::{self, Number, Value},
};
use quickjs_sys as sys;
use std::{
    ffi::{c_int, c_void, CString},
    mem::ManuallyDrop,
    rc::{Rc, Weak},
};

pub(crate) struct ContextInner {
    raw: *mut sys::JSContext,
    rt: Runtime,
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        unsafe {
            let opaque = sys::JS_GetContextOpaque(self.raw);
            if !opaque.is_null() {
                drop(Weak::from_raw(opaque as *const ContextInner));
            }
            sys::JS_FreeContext(self.raw);
        }
    }
}

#[derive(Clone)]
pub struct Context(Rc<ContextInner>);

impl Context {
    pub(crate) fn new(rt: Runtime) -> Self {
        let raw = unsafe {
            let raw = sys::JS_NewContext(rt.as_raw());
            sys::JS_AddIntrinsicRegExpCompiler(raw);
            raw
        };
        let inner = Rc::new(ContextInner { raw, rt });

        let weak = Rc::downgrade(&inner);
        unsafe { sys::JS_SetContextOpaque(raw, Weak::into_raw(weak) as *mut c_void) };

        Context(inner)
    }

    pub fn runtime(&self) -> &Runtime {
        &self.0.rt
    }

    pub fn global(&self) -> Value {
        let raw = unsafe { sys::JS_GetGlobalObject(self.as_raw()) };
        Value::from_raw(self.clone(), raw)
    }

    pub fn eval(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
        flags: i32,
    ) -> Result<Value, Error> {
        let source = source.as_ref();
        let c_source = CString::new(source).map_err(|_| Error::NulString)?;
        let c_name = CString::new(name.as_ref()).map_err(|_| Error::NulString)?;
        let raw = unsafe {
            sys::JS_Eval(
                self.as_raw(),
                c_source.as_ptr(),
                source.len(),
                c_name.as_ptr(),
                flags,
            )
        };
        self.check(Value::from_raw(self.clone(), raw))
    }

    pub fn eval_global(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Result<Value, Error> {
        self.eval(source, name, sys::JS_EVAL_TYPE_GLOBAL as i32)
    }

    pub fn eval_module(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Result<Value, Error> {
        const FLAGS: i32 = (sys::JS_EVAL_TYPE_MODULE | sys::JS_EVAL_FLAG_COMPILE_ONLY) as i32;
        self.eval(source, name, FLAGS)
    }

    pub fn make_bool(&self, value: bool) -> Value {
        Value::from_raw(self.clone(), value::mkval(sys::JS_TAG_BOOL, value as i32))
    }

    pub fn make_int(&self, value: i32) -> Value {
        Value::from_raw(self.clone(), value::mkval(sys::JS_TAG_INT, value))
    }

    pub fn make_float(&self, value: f64) -> Value {
        Value::from_raw(self.clone(), value::new_float64(self.as_raw(), value))
    }

    pub fn make_null(&self) -> Value {
        Value::from_raw(self.clone(), value::mkval(sys::JS_TAG_NULL, 0))
    }

    pub fn make_undefined(&self) -> Value {
        Value::from_raw(self.clone(), value::mkval(sys::JS_TAG_UNDEFINED, 0))
    }

    pub fn make_object(&self) -> Value {
        Value::from_raw(self.clone(), unsafe { sys::JS_NewObject(self.as_raw()) })
    }

    pub fn make_array(&self, values: impl IntoIterator<Item = Value>) -> Value {
        let raws: Vec<sys::JSValue> = values.into_iter().map(Value::into_raw).collect();
        let raw =
            unsafe { sys::JS_NewArrayFrom(self.as_raw(), raws.len() as c_int, raws.as_ptr()) };
        Value::from_raw(self.clone(), raw)
    }

    pub fn make_string(&self, value: impl AsRef<str>) -> Result<Value, Error> {
        let str = value.as_ref();
        let raw = unsafe { sys::JS_NewStringLen(self.as_raw(), str.as_ptr().cast(), str.len()) };
        self.check(Value::from_raw(self.clone(), raw))
    }

    pub fn make_buffer<T: Number + Clone>(&self, data: impl AsRef<[T]>) -> Result<Value, Error> {
        unsafe extern "C" fn free<T>(
            _: *mut sys::JSRuntime,
            opaque: *mut c_void,
            ptr: *mut c_void,
        ) {
            let capacity = opaque as usize;
            drop(unsafe { Vec::from_raw_parts(ptr.cast::<T>(), capacity, capacity) });
        }

        let mut buf = ManuallyDrop::new(data.as_ref().to_vec());
        let len = buf.len() * std::mem::size_of::<T>();
        let cap = buf.capacity();

        let raw = unsafe {
            sys::JS_NewArrayBuffer(
                self.as_raw(),
                buf.as_mut_ptr().cast::<u8>(),
                len,
                Some(free::<T>),
                cap as *mut c_void,
                false,
            )
        };
        self.check(Value::from_raw(self.clone(), raw))
    }

    pub(crate) fn check(&self, value: Value) -> Result<Value, Error> {
        if value.tag() == sys::JS_TAG_EXCEPTION {
            if self.runtime().state().interrupt_triggered() {
                // 被 deadline/cancel 中止：清掉 JS 侧待决异常，归一成 Interrupted。
                let _ = self.catch();
                return Err(Error::Interrupted);
            }
            Err(Error::Exception(self.catch()))
        } else {
            Ok(value)
        }
    }

    pub(crate) fn catch(&self) -> Exception {
        let raw = unsafe { sys::JS_GetException(self.as_raw()) };
        let value = Value::from_raw(self.clone(), raw);
        let value = value.borrow();

        let field = |key: &str| {
            let v = value.get_property_raw(key);
            if v.tag() == sys::JS_TAG_UNDEFINED || v.tag() == sys::JS_TAG_NULL {
                String::new()
            } else {
                v.borrow().to_string().unwrap_or_default()
            }
        };

        let name = field("name");
        let mut message = field("message");
        let stack = field("stack");

        if name.is_empty() && message.is_empty() {
            message = value.to_string().unwrap_or_default();
        }

        Exception {
            name,
            message,
            stack,
        }
    }

    pub(crate) fn throw(&self, message: &str) -> sys::JSValue {
        let ctx = self.as_raw();

        let error = unsafe { sys::JS_NewError(ctx) };
        if let Ok(msg) = self.make_string(message) {
            let atom = value::new_atom(ctx, "message");
            unsafe {
                sys::JS_SetProperty(ctx, error, atom, msg.into_raw());
                value::free_atom(ctx, atom);
            }
        }

        unsafe { sys::JS_Throw(ctx, error) }
    }

    pub(crate) unsafe fn from_opaque(raw: *mut sys::JSContext) -> Option<Context> {
        let opaque = sys::JS_GetContextOpaque(raw);
        if opaque.is_null() {
            return None;
        }

        let weak = Weak::from_raw(opaque as *const ContextInner);
        let upgraded = weak.upgrade().map(Context);
        let _ = Weak::into_raw(weak);

        upgraded
    }

    pub(crate) fn as_raw(&self) -> *mut sys::JSContext {
        self.0.raw
    }
}

#[cfg(feature = "async")]
impl Context {
    pub fn make_promise(&self) -> Result<(Value, Value, Value), Error> {
        let mut args = [value::mkval(sys::JS_TAG_UNDEFINED, 0); 2];
        let raw = unsafe { sys::JS_NewPromiseCapability(self.as_raw(), args.as_mut_ptr()) };

        let promise = self.check(Value::from_raw(self.clone(), raw))?;
        Ok((
            promise,
            Value::from_raw(self.clone(), args[0]),
            Value::from_raw(self.clone(), args[1]),
        ))
    }
}
