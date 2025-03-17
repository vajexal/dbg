use std::mem;

mod avl;
pub mod ranges;

pub const WORD_SIZE: usize = mem::size_of::<usize>();
