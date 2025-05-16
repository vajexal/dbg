mod breakpoint;
mod commands;
mod consts;
mod context;
mod debugger;
mod dwarf_parser;
mod error;
mod fsm;
mod loc_finder;
mod location;
mod path;
mod printer;
mod session;
mod trap;
mod types;
mod unwinder;
mod utils;
mod var;

use std::{io::Write, path::Path};

use error::DebuggerError;
use fsm::{CommandParser, Rule, FSM};

use anyhow::{bail, Result};
use debugger::Debugger;
use pest::Parser;

fn main() -> Result<()> {
    env_logger::init();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        bail!("pass program");
    }

    let prog_path = Path::new(&args[0]);

    let debugger = Debugger::new();
    let mut session = debugger.start(prog_path, &args[1..])?;
    let mut fsm = FSM::new(&mut session);

    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match CommandParser::parse(Rule::command, line) {
            Ok(pairs) => match fsm.handle(pairs) {
                Ok(should_quit) => {
                    if should_quit {
                        return Ok(());
                    }
                }
                Err(e) => match e.downcast_ref::<DebuggerError>() {
                    Some(_) => eprintln!("{}", e),
                    None => return Err(e),
                },
            },
            Err(e) => eprintln!("parser error {e}"),
        };
    }
}

fn readline() -> Result<String> {
    print!("> ");
    std::io::stdout().flush()?;
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    Ok(buf)
}
