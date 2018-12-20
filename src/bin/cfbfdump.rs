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

#[macro_use] // enable value_t! macro
extern crate clap;
extern crate evrecovery;

use std::io::{Read, Seek, Write, stdin, stdout, stderr};
use std::io::Error;
use std::fs::File;
use clap::{Arg, App, AppSettings, SubCommand, ArgMatches};
use evrecovery::cfbf::{Container, Object, ObjectResult, ObjectType};
use evrecovery::io::SeekableRead;
use evrecovery::io::Debug;

trait ReadSeek: Read + Seek { }
impl<T> ReadSeek for T where T: Read + Seek { }

fn main() {
	let matches = App::new("cfbfdump")
		.version("1.0")
		.author("Steve Muller <steve.muller@outlook.com>")
		.about("This utility reads a Compound File Binary File Format (also known as OLE file, COM file, or Structured Storage file) and dumps all contained files.")
		.setting(AppSettings::SubcommandRequired)
		.arg(Arg::with_name("verbose")
			.short("v")
			.help("Increases the debug verbosity. This will print a lot of debug messages to standard error (STDERR). Can be used up to 3 times.")
			.multiple(true)
			.takes_value(false))
		.subcommand(SubCommand::with_name("list")
			.about("Lists all files contained in the CFBF file. Each output line represents a file, and contains the internal file ID and the file path, separated by a space.")
			.arg(Arg::with_name("input")
				.value_name("FILE")
				.help("A file in Compound File Binary File Format (CFBF). If omitted, the file will be read from STDIN instead.")
				.short("i")
				.long("input")
				.required(false))
		)
		.subcommand(SubCommand::with_name("dump")
			.about("Dumps a stream from the CFBF file.")
			.arg(Arg::with_name("id")
				.value_name("STREAMID")
				.help("The ID of the stream that shall be dumped.")
				.long("id")
				.required(true))
			.arg(Arg::with_name("output")
				.value_name("FILE")
				.help("The file where the stream shall be written to. If this parameter is not specified (or has the value '-'), the stream will be written to STDOUT instead.")
				.short("o")
				.long("output")
				.required(false))
			.arg(Arg::with_name("input")
				.value_name("FILE")
				.help("A file in Compound File Binary File Format (CFBF). If omitted, the file will be read from STDIN instead.")
				.short("i")
				.long("input")
				.required(false))
		)
	.get_matches();

	let verbose = matches.occurrences_of("verbose") as i8;
	let mut debug = Debug::new(stderr(), verbose);

	if let Err(e) = dispatch(matches, &mut debug) {
		eprintln!("I/O ERROR: {}", e);
		std::process::exit(1);
	}
}

fn dispatch(matches: ArgMatches, debug: &mut Debug) -> Result<(), Error> {
	match matches.subcommand() {
		("list", Some(submatches)) => dispatch_list(submatches, debug),
		("dump", Some(submatches)) => dispatch_dump(submatches, debug),
		_ => panic!("Unrecognised subcommand"),
	}
}

fn dispatch_list(matches: &ArgMatches, debug: &mut Debug) -> Result<(), Error> {
	let inputfile = matches.value_of("input").unwrap_or("");
	let input: Box<ReadSeek> = match inputfile {
		"" | "-" => Box::new(SeekableRead::new(stdin())?),
		_ => Box::new(File::open(inputfile).unwrap())
	};
	let mut container = Container::new(input, debug)?;
	let root = container.get_root_object(debug)?;
	list_recursive(&mut container, &root, &mut String::from(""), debug)?;
	Ok(())
}

fn list_recursive<TFile>(container: &mut Container<TFile>, object: &Object, pathprefix: &mut String, debug: &mut Debug) -> Result<(), Error> where TFile: Read + Seek {
	let mut path = pathprefix.clone();
	if object.object_type != ObjectType::RootStorage {
		path.push_str("/");
		path.push_str(&object.name);
	}

	// Output object
	print!("{} {}", object.id, path);
	match object.object_type {
		ObjectType::Storage | ObjectType::RootStorage => println!("/"),
		ObjectType::Stream | _ => println!(),
	}

	// Output left sibling, if it exists
	if let ObjectResult::Ok(left_sibling_object) = container.get_left_sibling(object, debug)? {
		list_recursive(container, &left_sibling_object, pathprefix, debug)?;
	}

	// Output right sibling, if it exists
	if let ObjectResult::Ok(right_sibling_object) = container.get_right_sibling(object, debug)? {
		list_recursive(container, &right_sibling_object, pathprefix, debug)?;
	}

	// Output child, if it exists
	if let ObjectResult::Ok(child_object) = container.get_first_child(object, debug)? {
		list_recursive(container, &child_object, &mut path, debug)?;
	}

	Ok(())
}

fn dispatch_dump(matches: &ArgMatches, debug: &mut Debug) -> Result<(), Error> {
	let inputfile = matches.value_of("input").unwrap_or("");
	let outputfile = matches.value_of("output").unwrap_or("");
	let id = value_t!(matches, "id", u32).unwrap_or_else(|e| e.exit());

	let input: Box<ReadSeek> = match inputfile {
		"" | "-" => Box::new(SeekableRead::new(stdin())?),
		_ => Box::new(File::open(inputfile).unwrap())
	};
	let mut output: Box<Write> = match outputfile {
		"" | "-" => Box::new(stdout()),
		_ => Box::new(File::create(outputfile).unwrap())
	};
	let mut container = Container::new(input, debug)?;

	let object = container.get_object(id, debug)?;
	container.dump_stream(&object, &mut output, debug)?;
	Ok(())
}
