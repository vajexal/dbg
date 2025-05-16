use std::mem;

pub const WORD_SIZE: usize = mem::size_of::<usize>();

const _: () = assert!(WORD_SIZE == 8);

pub const MAIN_FUNC_NAME: &str = "main";

pub const FUNC_PROLOGUE_MAGIC_BYTES: [u8; 8] = [
    0xf3, 0x0f, 0x1e, 0xfa, // endbr64
    0x55, // push %rbp
    0x48, 0x89, 0xe5, // mov %rsp,%rbp
];
pub const FUNC_PROLOGUE_SIZE: usize = FUNC_PROLOGUE_MAGIC_BYTES.len();

/*
* ```
* 0: 5d   pop %rbp
* 1: c3   ret
* ```
*
* or
*
* ```
* 0: c9   leave
* 1: c3   ret
* ```
*/
pub const FUNC_EPILOGUE_SIZE: usize = 2;
