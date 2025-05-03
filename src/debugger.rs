use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process;

use crate::loc_finder::LocFinder;
use crate::session::DebugSession;
use crate::unwinder::{UnwindFrame, Unwinder};
use crate::utils::WORD_SIZE;
use gimli::Section;
use memmap2::Mmap;
use object::{Object, ObjectSection};
use typed_arena::Arena;

use anyhow::{anyhow, Result};
use nix::sys::{ptrace, wait};
use nix::unistd::Pid;

pub struct Debugger {
    arena_data: Arena<Vec<u8>>,
    arena_mmap: Arena<Mmap>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            arena_data: Arena::new(),
            arena_mmap: Arena::new(),
        }
    }

    pub fn start<I, S>(&self, prog: &Path, args: I) -> Result<DebugSession<gimli::EndianSlice<'_, gimli::RunTimeEndian>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let file = fs::File::open(prog)?;
        let map = self.arena_mmap.alloc(unsafe { Mmap::map(&file)? });
        let object = object::File::parse(&**map)?;

        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };

        let load_section = |section: gimli::SectionId| -> Result<gimli::EndianSlice<'_, _>> {
            let data = match object.section_by_name(section.name()) {
                Some(section) => match section.uncompressed_data()? {
                    Cow::Borrowed(b) => b,
                    Cow::Owned(b) => self.arena_data.alloc(b),
                },
                None => &[], // empty section
            };
            Ok(gimli::EndianSlice::new(data, endian))
        };

        let dwarf = gimli::Dwarf::load(load_section)?;
        let unwinder = Self::get_unwinder(&object, load_section)?;

        let mut command = process::Command::new(prog);

        unsafe {
            command.pre_exec(|| {
                ptrace::traceme()?;
                Ok(())
            });
        }

        let child = command.args(args).spawn()?;

        let base_address = match object.kind() {
            object::ObjectKind::Dynamic => Self::get_base_address(child.id())?,
            _ => 0,
        };
        log::trace!("base address {:#x}", base_address);

        let (loc_finder, type_storage) = LocFinder::make(&dwarf, base_address)?;

        wait::waitpid(Pid::from_raw(child.id() as libc::pid_t), None)?;

        Ok(DebugSession::new(child, dwarf, loc_finder, type_storage, unwinder, base_address))
    }

    fn get_unwinder<R, F>(object: &object::File, load_section: F) -> Result<Unwinder<R>>
    where
        R: gimli::Reader,
        F: Fn(gimli::SectionId) -> Result<R>,
    {
        let mut bases = gimli::BaseAddresses::default();
        if let Some(section) = object.section_by_name(gimli::SectionId::EhFrameHdr.name()) {
            bases = bases.set_eh_frame_hdr(section.address());
        }
        if let Some(section) = object.section_by_name(gimli::SectionId::EhFrame.name()) {
            bases = bases.set_eh_frame(section.address());
        }
        if let Some(section) = object.section_by_name(".text") {
            bases = bases.set_text(section.address());
        }
        if let Some(section) = object.section_by_name(".got") {
            bases = bases.set_got(section.address());
        }

        let parsed_eh_hdr_frame = match object.section_by_name(gimli::SectionId::EhFrameHdr.name()) {
            Some(_) => Some(gimli::EhFrameHdr::load(&load_section)?.parse(&bases, WORD_SIZE as u8)?),
            None => None,
        };

        let unwind_frame = match object.section_by_name(gimli::SectionId::DebugFrame.name()) {
            Some(_) => UnwindFrame::DebugFrame(gimli::DebugFrame::load(&load_section)?),
            None => UnwindFrame::EhFrame(gimli::EhFrame::load(&load_section)?, parsed_eh_hdr_frame),
        };

        Ok(Unwinder::new(unwind_frame, bases))
    }

    fn get_base_address(child_pid: u32) -> Result<u64> {
        let mut buf = vec![0; 16];
        let mut procmaps = fs::File::open(format!("/proc/{}/maps", child_pid))?;
        _ = procmaps.read(&mut buf)?;
        let (base_address, _) = std::str::from_utf8(&buf)?.split_once('-').ok_or(anyhow!("invalid proc maps"))?;
        let base_address = u64::from_str_radix(base_address, 16)?;

        Ok(base_address)
    }
}
