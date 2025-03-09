use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

use crate::commands;
use crate::debugger::Debugger;
use crate::debugger::DebuggerState;

pub struct FSM<'a, R: gimli::Reader> {
    debugger: &'a mut Debugger<R>,
}

impl<'a, R: gimli::Reader> FSM<'a, R> {
    pub fn new(debugger: &'a mut Debugger<R>) -> Self {
        Self { debugger }
    }

    pub fn handle(&mut self, command: Commands) -> Result<bool> {
        let should_quit = command == Commands::Quit;

        match self.debugger.get_state() {
            DebuggerState::Started => match command {
                Commands::Run => commands::control::run(self.debugger)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.debugger, loc)?,
                Commands::RemoveBreakpoint { loc } => commands::breakpoints::remove(self.debugger, &loc)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.debugger)?,
                Commands::EnableBreakpoint { loc } => commands::breakpoints::enable(self.debugger, &loc)?,
                Commands::DisableBreakpoint { loc } => commands::breakpoints::disable(self.debugger, &loc)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.debugger)?,
                Commands::Quit => commands::control::stop(self.debugger)?,
                _ => println!("invalid command"),
            },
            DebuggerState::Running => match command {
                Commands::Stop | Commands::Quit => commands::control::stop(self.debugger)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.debugger, loc)?,
                Commands::RemoveBreakpoint { loc } => commands::breakpoints::remove(self.debugger, &loc)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.debugger)?,
                Commands::EnableBreakpoint { loc } => commands::breakpoints::enable(self.debugger, &loc)?,
                Commands::DisableBreakpoint { loc } => commands::breakpoints::disable(self.debugger, &loc)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.debugger)?,
                Commands::Continue => commands::control::cont(self.debugger)?,
                Commands::Step => commands::control::step(self.debugger)?,
                Commands::StepIn => commands::control::step_in(self.debugger)?,
                Commands::StepOut => commands::control::step_out(self.debugger)?,
                Commands::PrintVar { name } => commands::print_var::print_var(self.debugger, name)?,
                _ => println!("invalid command"),
            },
            DebuggerState::Exited => match command {
                Commands::Quit => (),
                _ => println!("invalid command"),
            },
        }

        Ok(should_quit)
    }
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
pub struct Cli {
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
