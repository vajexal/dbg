use anyhow::{bail, Result};
use pest::iterators::Pairs;
use pest_derive::Parser;

use crate::commands;
use crate::error::DebuggerError;
use crate::path::{Path, PostfixOperator, PrefixOperator};
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
                Rule::help => commands::help::help(),
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
                Rule::print_var => {
                    let path = pair.into_inner().next().map(|pair| Self::parse_path(pair)).transpose()?;
                    commands::var::print_var(self.session, path.as_ref())?
                }
                Rule::set_var => {
                    let mut inner_pairs = pair.into_inner();
                    let path = Self::parse_path(inner_pairs.next().unwrap())?;
                    commands::var::set_var(self.session, &path, inner_pairs.next().unwrap().as_str())?
                }
                Rule::location => commands::control::location(self.session)?,
                Rule::help => commands::help::help(),
                _ => bail!(DebuggerError::InvalidCommand),
            },
            SessionState::Exited => match rule {
                Rule::quit => (),
                Rule::help => commands::help::help(),
                _ => bail!(DebuggerError::InvalidCommand),
            },
        }

        Ok(rule == Rule::quit)
    }

    fn parse_path(pair: pest::iterators::Pair<'_, Rule>) -> Result<Path<'_>> {
        if pair.as_rule() != Rule::path {
            bail!(DebuggerError::InvalidPath);
        }

        let mut path = Path::default();
        let mut pairs = pair.into_inner();

        for pair in pairs.by_ref() {
            match pair.as_rule() {
                Rule::operator => path.prefix_operators.push(PrefixOperator::try_from(pair.as_str())?),
                Rule::name => {
                    path.name = pair.as_str();
                    break;
                }
                _ => bail!(DebuggerError::InvalidPath),
            }
        }

        for pair in pairs {
            match pair.as_rule() {
                Rule::name => path.postfix_operators.push(PostfixOperator::Field(pair.as_str())),
                Rule::array_index => {
                    let index = pair.into_inner().next().unwrap().as_str().parse::<usize>()?;
                    path.postfix_operators.push(PostfixOperator::Index(index));
                }
                _ => bail!(DebuggerError::InvalidPath),
            }
        }

        Ok(path)
    }
}
