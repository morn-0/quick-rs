use crate::{
    context::Context,
    error::Error,
    value::{self, Value, ValueRef},
};
use quickjs_sys as sys;
use std::{
    any::TypeId,
    cell::RefCell,
    ffi::{c_int, c_void, CString},
    mem,
    panic::{self, AssertUnwindSafe},
    ptr,
};

pub type MutMethodFn<T> = fn(&mut T, &Context, ValueRef, &[ValueRef]) -> Result<Value, Error>;
pub type RefMethodFn<T> = fn(&T, &Context, ValueRef, &[ValueRef]) -> Result<Value, Error>;

pub type GetterFn<T> = fn(&T, &Context) -> Result<Value, Error>;
pub type SetterFn<T> = fn(&mut T, &Context, ValueRef) -> Result<(), Error>;

pub enum MethodFn<T> {
    Mut(MutMethodFn<T>),
    Ref(RefMethodFn<T>),
}

pub struct Method<T> {
    pub name: &'static str,
    pub argc: i32,
    pub func: MethodFn<T>,
}

pub struct GetSet<T> {
    pub name: &'static str,
    pub get: GetterFn<T>,
    pub set: Option<SetterFn<T>>,
}

pub trait JsClass: Sized + 'static {
    const NAME: &'static str;
    const CTOR_ARGC: i32 = 0;
    const METHODS: &'static [Method<Self>] = &[];
    const GETSETS: &'static [GetSet<Self>] = &[];

    fn constructor(ctx: &Context, args: &[ValueRef]) -> Result<Self, Error>;
}

impl Context {
    pub fn make_class<T: JsClass>(&self) -> Result<Value, Error> {
        let raw = self.as_raw();

        let id = match class_id_of::<T>(self) {
            Some(id) => id,
            None => {
                let name = CString::new(T::NAME).map_err(|_| Error::NulString)?;
                let def = sys::JSClassDef {
                    class_name: name.as_ptr(),
                    finalizer: Some(finalizer::<T>),
                    gc_mark: None,
                    call: None,
                    exotic: ptr::null_mut(),
                };

                let raw = self.runtime().as_raw();
                let mut id: sys::JSClassID = 0;

                unsafe {
                    sys::JS_NewClassID(raw, &mut id);
                    sys::JS_NewClass(raw, id, &def);
                }

                self.runtime()
                    .state()
                    .claxxs()
                    .borrow_mut()
                    .insert(TypeId::of::<T>(), id);
                id
            }
        };

        let proto = Value::from_raw(self.clone(), unsafe { sys::JS_NewObject(raw) });

        for method in T::METHODS {
            let func = match &method.func {
                MethodFn::Mut(f) => make_data_fn(
                    self,
                    Some(mut_method_trampoline::<T>),
                    method.argc,
                    *f as *mut c_void,
                ),
                MethodFn::Ref(f) => make_data_fn(
                    self,
                    Some(ref_method_trampoline::<T>),
                    method.argc,
                    *f as *mut c_void,
                ),
            };
            let name = CString::new(method.name).map_err(|_| Error::NulString)?;

            let ret = unsafe {
                sys::JS_DefinePropertyValueStr(
                    raw,
                    proto.raw(),
                    name.as_ptr(),
                    func.into_raw(),
                    (sys::JS_PROP_WRITABLE | sys::JS_PROP_CONFIGURABLE) as c_int,
                )
            };
            if ret < 0 {
                return Err(Error::Exception(self.catch()));
            }
        }

        for getset in T::GETSETS {
            let getter = make_data_fn(
                self,
                Some(getter_trampoline::<T>),
                0,
                getset.get as *mut c_void,
            );
            let setter = match getset.set {
                Some(set) => {
                    make_data_fn(self, Some(setter_trampoline::<T>), 1, set as *mut c_void)
                }
                None => self.make_undefined(),
            };

            let atom = value::new_atom(raw, getset.name);
            let ret = unsafe {
                sys::JS_DefinePropertyGetSet(
                    raw,
                    proto.raw(),
                    atom,
                    getter.into_raw(),
                    setter.into_raw(),
                    sys::JS_PROP_CONFIGURABLE as c_int,
                )
            };
            value::free_atom(raw, atom);
            if ret < 0 {
                return Err(Error::Exception(self.catch()));
            }
        }

        let name = CString::new(T::NAME).map_err(|_| Error::NulString)?;
        let ctor = unsafe {
            sys::JS_NewCFunction2(
                raw,
                Some(ctor_trampoline::<T>),
                name.as_ptr(),
                T::CTOR_ARGC,
                sys::JSCFunctionEnum_JS_CFUNC_constructor,
                0,
            )
        };
        let ctor = self.check(Value::from_raw(self.clone(), ctor))?;

        unsafe {
            sys::JS_SetConstructor(raw, ctor.raw(), proto.raw());
            sys::JS_SetClassProto(raw, id, proto.into_raw());
        }

        Ok(ctor)
    }
}

fn make_data_fn(ctx: &Context, func: sys::JSCFunctionData, argc: c_int, ptr: *mut c_void) -> Value {
    let mut data = value::mkptr(sys::JS_TAG_NULL, ptr);
    let raw = unsafe { sys::JS_NewCFunctionData(ctx.as_raw(), func, argc, 0, 1, &mut data) };
    Value::from_raw(ctx.clone(), raw)
}

fn class_id_of<T: JsClass>(ctx: &Context) -> Option<sys::JSClassID> {
    ctx.runtime()
        .state()
        .claxxs()
        .borrow()
        .get(&TypeId::of::<T>())
        .copied()
}

unsafe fn instance_of<'a, T: JsClass>(
    ctx: &Context,
    this: sys::JSValue,
) -> Result<&'a RefCell<T>, ()> {
    let id = class_id_of::<T>(ctx).ok_or(())?;
    let opaque = unsafe { sys::JS_GetOpaque2(ctx.as_raw(), this, id) };
    if opaque.is_null() {
        return Err(());
    }
    Ok(unsafe { &*(opaque as *const RefCell<T>) })
}

fn collect_args<'a>(ctx: &'a Context, argc: c_int, argv: *mut sys::JSValue) -> Vec<ValueRef<'a>> {
    (0..argc as isize)
        .map(|i| ValueRef {
            ctx,
            raw: unsafe { *argv.offset(i) },
        })
        .collect()
}

unsafe extern "C" fn ctor_trampoline<T: JsClass>(
    raw_ctx: *mut sys::JSContext,
    new_target: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };

    let args = collect_args(&ctx, argc, argv);
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| T::constructor(&ctx, &args)));
    let value = match outcome {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => return ctx.throw(&err.to_string()),
        Err(_) => return ctx.throw("rust constructor panicked"),
    };

    let id = match class_id_of::<T>(&ctx) {
        Some(id) => id,
        None => return ctx.throw("class not registered"),
    };

    let proto = match (ValueRef {
        ctx: &ctx,
        raw: new_target,
    })
    .get_property("prototype")
    {
        Ok(proto) => proto,
        Err(err) => return ctx.throw(&err.to_string()),
    };
    let obj = unsafe { sys::JS_NewObjectProtoClass(ctx.as_raw(), proto.raw(), id) };
    drop(proto);
    if value::tag_of(obj) == sys::JS_TAG_EXCEPTION {
        return obj;
    }

    let boxed = Box::into_raw(Box::new(RefCell::new(value)));
    unsafe { sys::JS_SetOpaque(obj, boxed as *mut c_void) };
    obj
}

unsafe extern "C" fn mut_method_trampoline<T: JsClass>(
    raw_ctx: *mut sys::JSContext,
    this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let cell = match unsafe { instance_of::<T>(&ctx, this) } {
        Ok(cell) => cell,
        Err(()) => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let func: MutMethodFn<T> = unsafe { mem::transmute(value::ptr_of(*data)) };

    let this_ref = ValueRef {
        ctx: &ctx,
        raw: this,
    };
    let args = collect_args(&ctx, argc, argv);
    let mut guard = match cell.try_borrow_mut() {
        Ok(guard) => guard,
        Err(_) => return ctx.throw("class instance is already borrowed (re-entrant access)"),
    };

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| func(&mut guard, &ctx, this_ref, &args)));
    match outcome {
        Ok(Ok(value)) => value.into_raw(),
        Ok(Err(err)) => ctx.throw(&err.to_string()),
        Err(_) => ctx.throw("rust method panicked"),
    }
}

unsafe extern "C" fn ref_method_trampoline<T: JsClass>(
    raw_ctx: *mut sys::JSContext,
    this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let cell = match unsafe { instance_of::<T>(&ctx, this) } {
        Ok(cell) => cell,
        Err(()) => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let func: RefMethodFn<T> = unsafe { mem::transmute(value::ptr_of(*data)) };

    let this_ref = ValueRef {
        ctx: &ctx,
        raw: this,
    };
    let args = collect_args(&ctx, argc, argv);
    let guard = match cell.try_borrow() {
        Ok(guard) => guard,
        Err(_) => return ctx.throw("class instance is already borrowed (re-entrant access)"),
    };

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| func(&guard, &ctx, this_ref, &args)));
    match outcome {
        Ok(Ok(value)) => value.into_raw(),
        Ok(Err(err)) => ctx.throw(&err.to_string()),
        Err(_) => ctx.throw("rust method panicked"),
    }
}

unsafe extern "C" fn getter_trampoline<T: JsClass>(
    raw_ctx: *mut sys::JSContext,
    this: sys::JSValue,
    _argc: c_int,
    _argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let cell = match unsafe { instance_of::<T>(&ctx, this) } {
        Ok(cell) => cell,
        Err(()) => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let func: GetterFn<T> = unsafe { mem::transmute(value::ptr_of(*data)) };

    let guard = match cell.try_borrow() {
        Ok(guard) => guard,
        Err(_) => return ctx.throw("class instance is already borrowed (re-entrant access)"),
    };

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| func(&guard, &ctx)));
    match outcome {
        Ok(Ok(value)) => value.into_raw(),
        Ok(Err(err)) => ctx.throw(&err.to_string()),
        Err(_) => ctx.throw("rust getter panicked"),
    }
}

unsafe extern "C" fn setter_trampoline<T: JsClass>(
    raw_ctx: *mut sys::JSContext,
    this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let cell = match unsafe { instance_of::<T>(&ctx, this) } {
        Ok(cell) => cell,
        Err(()) => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };
    let func: SetterFn<T> = unsafe { mem::transmute(value::ptr_of(*data)) };

    let value = ValueRef {
        ctx: &ctx,
        raw: if argc >= 1 {
            unsafe { *argv }
        } else {
            value::mkval(sys::JS_TAG_UNDEFINED, 0)
        },
    };
    let mut guard = match cell.try_borrow_mut() {
        Ok(guard) => guard,
        Err(_) => return ctx.throw("class instance is already borrowed (re-entrant access)"),
    };

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| func(&mut guard, &ctx, value)));
    match outcome {
        Ok(Ok(())) => value::mkval(sys::JS_TAG_UNDEFINED, 0),
        Ok(Err(err)) => ctx.throw(&err.to_string()),
        Err(_) => ctx.throw("rust setter panicked"),
    }
}

unsafe extern "C" fn finalizer<T: JsClass>(_rt: *mut sys::JSRuntime, val: sys::JSValue) {
    let id = unsafe { sys::JS_GetClassID(val) };
    let opaque = unsafe { sys::JS_GetOpaque(val, id) };
    if !opaque.is_null() {
        drop(unsafe { Box::from_raw(opaque as *mut RefCell<T>) });
    }
}
