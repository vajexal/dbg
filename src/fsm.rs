use anyhow::{bail, Result};
use pest::iterators::Pairs;
use pest_derive::Parser;

use crate::commands;
use crate::error::DebuggerError;
use crate::session::{DebugSession, SessionState};

#[derive(Parser)]
#[grammar = "parser.pest"]
pub struct CommandParser;

#[allow(clippy::upper_case_acronyms)]
pub struct FSM<'a, R: gimli::Reader> {
    session: &'a mut DebugSession<R>,
}

impl<'a, R: gimli::Reader> FSM<'a, R> {
    pub fn new(debugger: &'a mut DebugSession<R>) -> Self {
        Self { session: debugger }
    }

    pub fn handle(&mut self, mut pairs: Pairs<Rule>) -> Result<bool> {
        let pair = pairs.next().unwrap().into_inner().next().unwrap();
        let rule = pair.as_rule();

        match self.session.get_state() {
            SessionState::Started => match rule {
                Rule::run => commands::control::run(self.session)?,
                Rule::add_breakpoint => commands::breakpoints::add(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::remove_breakpoint => commands::breakpoints::remove(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::list_breakpoints => commands::breakpoints::list(self.session)?,
                Rule::enable_breakpoint => commands::breakpoints::enable(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::disable_breakpoint => commands::breakpoints::disable(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::clear_breakpoints => commands::breakpoints::clear(self.session)?,
                Rule::quit => commands::control::stop(self.session)?,
                _ => bail!(DebuggerError::InvalidCommand),
            },
            SessionState::Running => match rule {
                Rule::stop | Rule::quit => commands::control::stop(self.session)?,
                Rule::add_breakpoint => commands::breakpoints::add(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::remove_breakpoint => commands::breakpoints::remove(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::list_breakpoints => commands::breakpoints::list(self.session)?,
                Rule::enable_breakpoint => commands::breakpoints::enable(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::disable_breakpoint => commands::breakpoints::disable(self.session, pair.into_inner().next().unwrap().as_str())?,
                Rule::clear_breakpoints => commands::breakpoints::clear(self.session)?,
                Rule::r#continue => commands::control::cont(self.session)?,
                Rule::step => commands::control::step(self.session)?,
                Rule::step_in => commands::control::step_in(self.session)?,
                Rule::step_out => commands::control::step_out(self.session)?,
                Rule::print_var => commands::var::print_var(self.session, pair.into_inner().next().map(|pair| pair.as_str()))?,
                Rule::set_var => {
                    let mut inner_pairs = pair.into_inner();
                    commands::var::set_var(self.session, inner_pairs.next().unwrap().as_str(), inner_pairs.next().unwrap().as_str())?
                }
                _ => bail!(DebuggerError::InvalidCommand),
            },
            SessionState::Exited => match rule {
                Rule::quit => (),
                _ => bail!(DebuggerError::InvalidCommand),
            },
        }

        Ok(rule == Rule::quit)
    }
}
