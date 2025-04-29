pub struct Context {
    pub ip: u64,
    pub sp: u64,
    pub bp: u64,
}

impl Context {
    pub fn new(regs: libc::user_regs_struct) -> Self {
        Self {
            ip: regs.rip,
            sp: regs.rsp,
            bp: regs.rbp,
        }
    }
}
