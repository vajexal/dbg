mod breakpoint;
mod commands;
mod debugger;
mod error;
mod fsm;
mod loc_finder;
mod printer;
mod session;
mod trap;
mod unwinder;
mod utils;
mod var;

use std::{io::Write, path::Path};

use error::DebuggerError;
use fsm::{CommandParser, FSM};

use anyhow::{bail, Result};
use clap::Parser;
use debugger::Debugger;

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

        let args = match shlex::split(line) {
            Some(args) => args,
            None => {
                eprintln!("parse line");
                continue;
            }
        };
        let parser = match CommandParser::try_parse_from(args) {
            Ok(cli) => cli,
            Err(e) => {
                eprintln!("parse comamnd {e}");
                continue;
            }
        };

        match fsm.handle(parser.command) {
            Ok(should_quit) => {
                if should_quit {
                    break;
                }
            }
            Err(e) => match e.downcast_ref::<DebuggerError>() {
                Some(_) => eprintln!("{}", e),
                None => return Err(e),
            },
        };
    }

    Ok(())
}

fn readline() -> Result<String> {
    print!("> ");
    std::io::stdout().flush()?;
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    Ok(buf)
}
