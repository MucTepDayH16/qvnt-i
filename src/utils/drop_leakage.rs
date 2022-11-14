pub fn leak_string<'t>(s: String) -> &'t str {
    let s = Box::leak(s.into_boxed_str()) as &'t str;
    #[cfg(test)]
    eprintln!("Leakage {{ ptr: {:?}, len: {} }}", s as *const _, s.len());
    s
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
            #[cfg(test)]
            eprintln!("Unleak {{ ptr: {:?}, len: {} }}", s as *const _, s.len());
            std::mem::drop(Box::from_raw(s as *const str as *mut str));
        }
    }
}
