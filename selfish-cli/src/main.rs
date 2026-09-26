//! Command-line access to the formats the library crates read and write.
//!
//! Each subcommand is a thin layer over a library crate. A behaviour a command needs belongs in
//! the crate, so every consumer gets it.

#![forbid(unsafe_code)]

use clap::Parser;

use crate::args::{Cli, Command};

/// Print a line, and exit zero when the reader has gone away.
///
/// `println!` panics on a closed pipe, so `selfish imports big.prx | head` would end in a
/// backtrace. A reader that stopped reading got what it wanted, so the exit status is zero.
/// Restoring the default `SIGPIPE` handler instead needs `unsafe`, which this crate forbids.
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let mut out = std::io::stdout().lock();
        if writeln!(out, $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

mod args;
mod container;
mod elf;
mod icon;
mod layout;
mod pack;
mod package;
mod pipeline;
mod shader;
mod title;

/// The result every command returns.
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Print the error's `Display`, which says what to do about it, and exit non-zero. (D095)
fn main() {
    if let Err(error) = run() {
        eprintln!("selfish: {error}");
        std::process::exit(1);
    }
}

/// A pipeline invocation when `--input` is given, otherwise one subcommand.
fn run() -> Result {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        if cli.input.is_some() {
            return pipeline::run(&cli);
        }
        <Cli as clap::CommandFactory>::command().print_help()?;
        return Ok(());
    };

    match command {
        Command::Nid { names } => elf::nid(&names),
        Command::Elf { file } => elf::describe(&file),
        Command::Imports { file, all } => elf::imports(&file, all),
        Command::Sections { file, defines } => elf::sections(&file, &defines),
        Command::Reloc { file } => elf::reloc(&file),
        Command::Container { file } => container::describe(&file),
        Command::Title { file, round_trip } => title::describe(&file, round_trip),
        Command::Audit { file } => container::audit(&file),
        Command::Image {
            root,
            out,
            content_id,
            passcode,
        } => pack::image(&root, &out, &content_id, passcode.as_deref()),
        Command::Pack {
            image,
            dir,
            passcode,
            out,
            content_id,
            entries,
            title_id,
            title,
            version,
        } => pack::pack(&pack::Request {
            image: image.as_deref(),
            dir: dir.as_deref(),
            passcode: passcode.as_deref(),
            out: &out,
            content_id: &content_id,
            entries: &entries,
            title_id: title_id.as_deref(),
            title: title.as_deref(),
            version: &version,
        }),
        Command::Derive { files } => package::derive(&files),
        Command::Pkg { file, all } => package::list(&file, all),
        Command::Extract { file, out } => package::extract(&file, &out),
        Command::Shader {
            stage,
            code,
            shader_size,
            target,
            sh_registers,
            out,
        } => shader::build(&shader::Request {
            stage: &stage,
            code: code.as_deref(),
            shader_size,
            target: &target,
            sh_registers: &sh_registers,
            out: &out,
        }),
    }
}
