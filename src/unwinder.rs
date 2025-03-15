use std::{cell::RefCell, ops::DerefMut};

use gimli::UnwindSection;

pub enum UnwindFrame<R: gimli::Reader> {
    DebugFrame(gimli::DebugFrame<R>),
    EhFrame(gimli::EhFrame<R>),
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
                debug_frame.unwind_info_for_address(&self.bases, ctx.deref_mut(), relative_address, gimli::DebugFrame::cie_from_offset)
            }
            // todo binary search
            UnwindFrame::EhFrame(eh_frame) => eh_frame.unwind_info_for_address(&self.bases, ctx.deref_mut(), relative_address, gimli::EhFrame::cie_from_offset),
        }?;

        Ok(unwind_info.cfa().clone())
    }

    pub fn unwind_expression(&self, unwind_expression: &gimli::UnwindExpression<R::Offset>) -> gimli::Result<gimli::Expression<R>> {
        match &self.unwind_frame {
            UnwindFrame::DebugFrame(debug_frame) => unwind_expression.get(debug_frame),
            UnwindFrame::EhFrame(eh_frame) => unwind_expression.get(eh_frame),
        }
    }
}
