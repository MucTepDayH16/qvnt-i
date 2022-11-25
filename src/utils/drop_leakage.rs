pub fn leak_string<'t>(s: String) -> &'t str {
    let s = Box::leak(s.into_boxed_str()) as &'t str;
    log::trace!(target: "qvnt_i::drop_leakage","Leakage {{ ptr: {:?}, len: {} }}", s as *const _, s.len());
    s
}

/// *Safety*: `s` should be the string returned from `leak_string`
pub unsafe fn unleak_str(s: &'_ str) {
    std::mem::drop(Box::from_raw(s as *const str as *mut str));
}

pub trait DropExt {
    fn drop(self);
}

impl<'t> DropExt for qvnt::qasm::Int<'t> {
    fn drop(self) {
        self.into_iter_ast().for_each(DropExt::drop);
    }
}

impl<'t> DropExt for qvnt::qasm::Ast<'t> {
    fn drop(self) {
        unsafe {
            let s = self.source();
            log::trace!(target: "qvnt_i::drop_leakage", "Unleak {{ ptr: {:?}, len: {} }}", s as *const _, s.len());
            unleak_str(s);
        }
    }
}
