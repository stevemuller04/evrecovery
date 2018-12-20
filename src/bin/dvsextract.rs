/*
evrecovery library & toolset
Copyright (C) 2018 Steve Muller <steve.muller@outlook.com>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

extern crate clap;
extern crate evrecovery;

use std::io::{stdin, stdout, stderr};
use std::io::{Read, Write, BufReader, BufWriter};
use std::io::Error;
use std::fs::File as File;
use clap::{Arg, App};
use evrecovery::dvs::File as DvsFile;
use evrecovery::io::Debug;

fn main() {
	let matches = App::new("dvsextract")
		.version("1.0")
		.author("Steve Muller <steve.muller@outlook.com>")
		.about("This utility reads an Enterprise Vault archive file (.dvs) and extracts the archived data.")
		.arg(Arg::with_name("verbose")
			.short("v")
			.help("Increases the debug verbosity. This will print a lot of debug messages to standard error (STDERR). Can be used up to 3 times.")
			.multiple(true)
			.takes_value(false))
		.arg(Arg::with_name("input")
			.short("i")
			.long("input")
			.value_name("FILE")
			.help("If specified, then the .dvs file will be read from this file. Otherwise it will be read from standard input (STDIN).")
			.takes_value(true))
		.arg(Arg::with_name("output")
			.short("o")
			.long("output")
			.value_name("FILE")
			.help("If specified, then the original file will be written to this file. Otherwise it will be written to standard output (STDOUT).")
			.takes_value(true))
	.get_matches();

	let verbose = matches.occurrences_of("verbose") as i8;
	let inputfile = matches.value_of("input").unwrap_or("-");
	let outputfile = matches.value_of("output").unwrap_or("-");

	let mut debug = Debug::new(stderr(), verbose);
	let input: Box<Read> = match inputfile {
		"" | "-" => Box::new(stdin()),
		_ => Box::new(File::open(inputfile).unwrap())
	};
	let output: Box<Write> = match outputfile {
		"" | "-" => Box::new(stdout()),
		_ => Box::new(File::create(outputfile).unwrap())
	};

	if let Err(e) = process(BufReader::new(input), BufWriter::new(output), &mut debug) {
		eprintln!("I/O ERROR: {}", e);
		std::process::exit(1);
	}
}

fn process(input: impl Read, mut output: impl Write, debug: &mut Debug) -> Result<(), Error> {
	let file = DvsFile::new(input, debug)?;
	file.decompress(&mut output, debug)?;
	Ok(())
}
