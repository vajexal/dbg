use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

use crate::commands;
use crate::session::{DebugSession, SessionState};

#[allow(clippy::upper_case_acronyms)]
pub struct FSM<'a, R: gimli::Reader> {
    session: &'a mut DebugSession<R>,
}

impl<'a, R: gimli::Reader> FSM<'a, R> {
    pub fn new(debugger: &'a mut DebugSession<R>) -> Self {
        Self { session: debugger }
    }

    pub fn handle(&mut self, command: Commands) -> Result<bool> {
        let should_quit = command == Commands::Quit;

        match self.session.get_state() {
            SessionState::Started => match command {
                Commands::Run => commands::control::run(self.session)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.session, loc)?,
                Commands::RemoveBreakpoint { loc } => commands::breakpoints::remove(self.session, &loc)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.session)?,
                Commands::EnableBreakpoint { loc } => commands::breakpoints::enable(self.session, &loc)?,
                Commands::DisableBreakpoint { loc } => commands::breakpoints::disable(self.session, &loc)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.session)?,
                Commands::Quit => commands::control::stop(self.session)?,
                _ => println!("invalid command"),
            },
            SessionState::Running => match command {
                Commands::Stop | Commands::Quit => commands::control::stop(self.session)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.session, loc)?,
                Commands::RemoveBreakpoint { loc } => commands::breakpoints::remove(self.session, &loc)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.session)?,
                Commands::EnableBreakpoint { loc } => commands::breakpoints::enable(self.session, &loc)?,
                Commands::DisableBreakpoint { loc } => commands::breakpoints::disable(self.session, &loc)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.session)?,
                Commands::Continue => commands::control::cont(self.session)?,
                Commands::Step => commands::control::step(self.session)?,
                Commands::StepIn => commands::control::step_in(self.session)?,
                Commands::StepOut => commands::control::step_out(self.session)?,
                Commands::PrintVar { name } => commands::print_var::print_var(self.session, name)?,
                _ => println!("invalid command"),
            },
            SessionState::Exited => match command {
                Commands::Quit => (),
                _ => println!("invalid command"),
            },
        }

        Ok(should_quit)
    }
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
pub struct CommandParser {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    #[command(visible_alias = "r")]
    Run,
    Stop,
    #[command(name = "breakpoint", visible_alias = "break", visible_alias = "b")]
    AddBreakpoint {
        loc: String,
    },
    #[command(name = "remove", visible_alias = "rm")]
    RemoveBreakpoint {
        loc: String,
    },
    #[command(name = "list", visible_alias = "l")]
    ListBreakpoints,
    #[command(name = "enable")]
    EnableBreakpoint {
        loc: String,
    },
    #[command(name = "disable")]
    DisableBreakpoint {
        loc: String,
    },
    #[command(name = "clear")]
    ClearBreakpoints,
    #[command(visible_alias = "cont", visible_alias = "c")]
    Continue,
    Step,
    StepIn,
    StepOut,
    #[command(name = "print", visible_alias = "p")]
    PrintVar {
        name: Option<String>,
    },
    #[command(visible_alias = "q")]
    Quit,
}
