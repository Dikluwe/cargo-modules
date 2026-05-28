#![allow(dead_code, unused_variables)]

pub struct S;

impl S {
    pub fn duplicated(&self) {}
}

pub trait T {
    fn duplicated(&self);
}

impl T for S {
    fn duplicated(&self) {}
}
