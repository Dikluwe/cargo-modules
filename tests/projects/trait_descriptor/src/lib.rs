#![allow(dead_code, unused_variables)]

pub struct S;

pub struct A;
pub struct B;

// Inherent impl: its method must have `trait: null`.
impl S {
    pub fn inherent(&self) {}
}

// Two distinct traits sharing a method name. The two `shared` methods collide
// on `path` and are told apart by the `trait` field (the analogue of the
// `Display::fmt` vs `Debug::fmt` collision, but with user-defined traits so
// the test does not depend on `--sysroot`).
pub trait Alpha {
    fn shared(&self);
}

pub trait Beta {
    fn shared(&self);
}

impl Alpha for S {
    fn shared(&self) {}
}

impl Beta for S {
    fn shared(&self) {}
}

// One generic trait implemented twice with different arguments. The bare
// `trait` is "Convert" for both; only `trait_ref` ("Convert<A>" vs
// "Convert<B>") distinguishes them (the analogue of `From<A>` vs `From<B>`,
// again user-defined to avoid a sysroot dependency).
pub trait Convert<T> {
    fn convert(&self) -> T;
}

impl Convert<A> for S {
    fn convert(&self) -> A {
        A
    }
}

impl Convert<B> for S {
    fn convert(&self) -> B {
        B
    }
}

// Modifier flags.
pub const fn a_const_fn() {}

pub async fn an_async_fn() {}

pub unsafe fn an_unsafe_fn() {}

// `#[non_exhaustive]`.
#[non_exhaustive]
pub enum NonExhaustiveEnum {
    Variant,
}

// Generics + bounds (for `--rich`). The bound is a user-defined trait so the
// bound name resolves without `--sysroot`.
pub fn generic_fn<T: Alpha>(value: T) {}
