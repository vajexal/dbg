use std::borrow::Cow;
use std::fs;
use std::path::Path;

use anyhow::Result;
use gimli::Section;
use memmap2::Mmap;
use object::{Object, ObjectSection};
use typed_arena::Arena;

use crate::unwinder::{UnwindFrame, Unwinder};
use crate::utils::WORD_SIZE;

pub struct Loader {
    arena_data: Arena<Vec<u8>>,
    arena_mmap: Arena<Mmap>,
}

impl Loader {
    pub fn new() -> Self {
        Self {
            arena_data: Arena::new(),
            arena_mmap: Arena::new(),
        }
    }

    pub fn load(
        &self,
        prog: &Path,
    ) -> Result<(
        gimli::Dwarf<gimli::EndianSlice<'_, gimli::RunTimeEndian>>,
        Unwinder<gimli::EndianSlice<'_, gimli::RunTimeEndian>>,
        object::ObjectKind,
    )> {
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
            Some(_) => Some(gimli::EhFrameHdr::load(load_section)?.parse(&bases, WORD_SIZE as u8)?),
            None => None,
        };

        let unwind_frame = match object.section_by_name(gimli::SectionId::DebugFrame.name()) {
            Some(_) => UnwindFrame::DebugFrame(gimli::DebugFrame::load(load_section)?),
            None => UnwindFrame::EhFrame(gimli::EhFrame::load(load_section)?, parsed_eh_hdr_frame),
        };
        let unwinder = Unwinder::new(unwind_frame, bases);

        Ok((dwarf, unwinder, object.kind()))
    }
}
