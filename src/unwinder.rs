use std::cell::RefCell;

use gimli::UnwindSection;

pub enum UnwindFrame<R: gimli::Reader> {
    DebugFrame(gimli::DebugFrame<R>),
    EhFrame(gimli::EhFrame<R>, Option<gimli::ParsedEhFrameHdr<R>>),
}

pub struct Unwinder<R: gimli::Reader> {
    unwind_frame: UnwindFrame<R>,
    ctx: RefCell<gimli::UnwindContext<R::Offset>>,
    bases: gimli::BaseAddresses,
}

impl<R: gimli::Reader> Unwinder<R> {
    pub fn new(unwind_frame: UnwindFrame<R>, bases: gimli::BaseAddresses) -> Self {
        Self {
            unwind_frame,
            ctx: RefCell::new(gimli::UnwindContext::new()),
            bases,
        }
    }

    pub fn unwind_cfa(&self, relative_address: u64) -> gimli::Result<gimli::CfaRule<R::Offset>> {
        let mut ctx = self.ctx.borrow_mut();

        let unwind_info = match &self.unwind_frame {
            UnwindFrame::DebugFrame(debug_frame) => {
                debug_frame.unwind_info_for_address(&self.bases, &mut ctx, relative_address, gimli::DebugFrame::cie_from_offset)
            }
            UnwindFrame::EhFrame(eh_frame, parsed_eh_frame_hdr) => {
                match parsed_eh_frame_hdr.as_ref().and_then(|parsed_eh_frame_hdr| parsed_eh_frame_hdr.table()) {
                    Some(eh_hdr_table) => {
                        eh_hdr_table.unwind_info_for_address(eh_frame, &self.bases, &mut ctx, relative_address, gimli::EhFrame::cie_from_offset)
                    }
                    None => eh_frame.unwind_info_for_address(&self.bases, &mut ctx, relative_address, gimli::EhFrame::cie_from_offset),
                }
            }
        }?;

        Ok(unwind_info.cfa().clone())
    }

    pub fn unwind_expression(&self, unwind_expression: &gimli::UnwindExpression<R::Offset>) -> gimli::Result<gimli::Expression<R>> {
        match &self.unwind_frame {
            UnwindFrame::DebugFrame(debug_frame) => unwind_expression.get(debug_frame),
            UnwindFrame::EhFrame(eh_frame, _) => unwind_expression.get(eh_frame),
        }
    }
}
