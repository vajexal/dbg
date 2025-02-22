mod commands;
mod debugger;
mod fsm;
mod loader;
mod loc_finder;
mod unwinder;
mod utils;
mod var;
mod printer;
mod error;

use std::{io::Write, path::Path};

use fsm::{Cli, FSM};
use loader::Loader;

use anyhow::{bail, Result};
use clap::Parser;
use debugger::Debugger;

fn main() -> Result<()> {
    env_logger::init();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() == 0 {
        bail!("pass program");
    }

    let prog_path = Path::new(&args[0]);

    let loader = Loader::new();
    let (dwarf, unwinder) = loader.load(prog_path)?;
    let mut debugger = Debugger::start(prog_path, &args[1..], dwarf, unwinder)?;
    let mut fsm = FSM::new(&mut debugger);

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
        let cli = match Cli::try_parse_from(args) {
            Ok(cli) => cli,
            Err(e) => {
                eprintln!("parse comamnd {e}");
                continue;
            }
        };

        if fsm.handle(cli.command)? {
            break;
        }
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
