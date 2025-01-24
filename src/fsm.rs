use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

use crate::commands;
use crate::debugger::Debugger;
use crate::debugger::DebuggerState;

pub struct FSM<'a, R: gimli::Reader> {
    debugger: &'a mut Debugger<R>,
    state: DebuggerState,
}

impl<'a, R: gimli::Reader> FSM<'a, R> {
    pub fn new(debugger: &'a mut Debugger<R>) -> Self {
        Self {
            debugger,
            state: DebuggerState::Exited,
        }
    }

    pub fn handle(&mut self, command: Commands) -> Result<bool> {
        let should_quit = command == Commands::Quit;

        match self.state {
            DebuggerState::Exited => match command {
                Commands::Run => self.state = commands::control::run(self.debugger)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.debugger, loc)?,
                Commands::RemoveBreakpoint { index } => commands::breakpoints::remove(self.debugger, index)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.debugger)?,
                Commands::EnableBreakpoint { index } => commands::breakpoints::enable(self.debugger, index)?,
                Commands::DisableBreakpoint { index } => commands::breakpoints::disable(self.debugger, index)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.debugger)?,
                Commands::Quit => (),
                _ => println!("invalid command"),
            },
            DebuggerState::Running => match command {
                Commands::Stop | Commands::Quit => self.state = commands::control::stop(self.debugger)?,
                Commands::AddBreakpoint { loc } => commands::breakpoints::add(self.debugger, loc)?,
                Commands::RemoveBreakpoint { index } => commands::breakpoints::remove(self.debugger, index)?,
                Commands::ListBreakpoints => commands::breakpoints::list(self.debugger)?,
                Commands::EnableBreakpoint { index } => commands::breakpoints::enable(self.debugger, index)?,
                Commands::DisableBreakpoint { index } => commands::breakpoints::disable(self.debugger, index)?,
                Commands::ClearBreakpoints => commands::breakpoints::clear(self.debugger)?,
                Commands::Continue => self.state = commands::control::cont(self.debugger)?,
                Commands::Step => self.state = commands::control::step(self.debugger)?,
                Commands::StepIn => self.state = commands::control::step_in(self.debugger)?,
                Commands::StepOut => self.state = commands::control::step_out(self.debugger)?,
                Commands::PrintVar { name } => commands::print_var::print_var(self.debugger, name)?,
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
    #[command(visible_alias = "rm")]
    RemoveBreakpoint {
        index: usize,
    },
    #[command(visible_alias = "l")]
    ListBreakpoints,
    #[command(name = "enable")]
    EnableBreakpoint {
        index: usize,
    },
    #[command(name = "disable")]
    DisableBreakpoint {
        index: usize,
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
