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

use std::io::{Read, Seek, Write, stdin, stdout, stderr};
use std::io::{Error, ErrorKind};
use std::io::copy;
use std::fs::{File, create_dir_all};
use std::path::{Component, PathBuf};
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use clap::{Arg, App};
use evrecovery::cfbf::{Container, ObjectResult};
use evrecovery::io::SeekableRead;
use evrecovery::dvs::File as DvsFile;
use evrecovery::io::Debug;

trait ReadSeek: Read + Seek { }
impl<T> ReadSeek for T where T: Read + Seek { }

fn main() {
	let matches = App::new("dvsrestore")
		.version("1.0")
		.author("Steve Muller <steve.muller@outlook.com>")
		.about("This utility reads an Enterprise Vault DVS File and restores the contained archived file.")
		.arg(Arg::with_name("verbose")
			.short("v")
			.help("Increases the debug verbosity. This will print a lot of debug messages to standard error (STDERR). Can be used up to 4 times.")
			.multiple(true)
			.takes_value(false))
		.arg(Arg::with_name("input")
			.value_name("FILE")
			.help("A DVS file. If omitted, the file will be read from STDIN instead.")
			.required(false))
		.arg(Arg::with_name("path-only")
			.long("path-only")
			.help("If set, only the original path (including the file name) is output to STDOUT, and no content is recovered.")
			.takes_value(false)
			.required(false))
		.arg(Arg::with_name("target")
			.value_name("TARGETDIR")
			.help("The path of the directory where the archived file shall be extracted to, under its original path/filename. If the target directory is not specified, the file will be output to STDOUT instead.")
			.short("t")
			.long("target")
			.required(false))
		.arg(Arg::with_name("ext")
			.value_name("FILEEXT")
			.help("The file extension that shall be used for outsourced files. The full name of the outsourced file will be constructed as '<file name of the dvs file>.<ext>'.")
			.long("ext")
			.default_value("dvf")
			.required(false))
	.get_matches();

	let verbose = matches.occurrences_of("verbose") as i8 - 1;
	let inputfile = matches.value_of("input").unwrap_or("");
	let target_dir = matches.value_of("target").unwrap_or("");
	let pathonly = matches.occurrences_of("path-only") > 0;
	let outsourced_extension = matches.value_of("ext").unwrap();

	let mut debug = Debug::new(stderr(), verbose);
	let input: Box<Read> = match inputfile {
		"" | "-" => Box::new(stdin()),
		_ => Box::new(File::open(inputfile).unwrap())
	};

	let inputfile_outsourced: Option<String> = match inputfile {
		"" | "-" => Option::None,
		_ => if inputfile.to_lowercase().ends_with(".dvs") { Option::Some(format!("{}.{}", &inputfile[..inputfile.len()-4], outsourced_extension)) } else { Option::None },
	};

	if let Err(e) = process(input, target_dir, pathonly, inputfile_outsourced, &mut debug) {
		eprintln!("I/O ERROR: {}", e);
		std::process::exit(1);
	}
}

fn process(input: impl Read, target_dir: &str, pathonly: bool, inputfile_outsourced: Option<String>, debug: &mut Debug) -> Result<(), Error> {
	match pathonly {
		true => process_info(input, debug),
		false => process_dump(input, target_dir, inputfile_outsourced, debug),
	}
}

fn process_info(input: impl Read, debug: &mut Debug) -> Result<(), Error> {
	// First extract the CFBF file
	debug.logln(0, format!("Reading DVS file ..."));
	let mut cfbfdata: Vec<u8> = Vec::new();
	DvsFile::new(input, debug)?.decompress(&mut cfbfdata, debug)?;
	debug.logln(0, format!("Read DVS file."));

	// Parse CFBF file
	debug.logln(0, format!("Reading CFBF file ..."));
	let mut container = Container::new(SeekableRead::new(&cfbfdata[..])?, debug)?;
	debug.logln(0, format!("Read CFBF file."));

	// Get information
	let original_path_dir = read_path_from_embedded_file(&mut container, &["Root Entry".to_owned(), "User Information".to_owned(), "Location".to_owned(), "ExchangeLocation".to_owned(), "FolderPath".to_owned()], false, false, debug)?;
	debug.logln(0, format!("Original directory: {:?}", original_path_dir));
	let original_path_file = read_path_from_embedded_file(&mut container, &["Root Entry".to_owned(), "User Information".to_owned(), "User Archivable Item".to_owned(), "Title".to_owned()], true, false, debug)?;
	debug.logln(0, format!("Original file name: {:?}", original_path_file));

	// Build original path
	let mut original_path = PathBuf::from(original_path_dir);
	original_path.push(&original_path_file);
	println!("{}", original_path.to_str().unwrap());
	Ok(())
}

fn process_dump(input: impl Read, target_dir: &str, inputfile_outsourced: Option<String>, debug: &mut Debug) -> Result<(), Error> {
	// First extract the CFBF file
	debug.logln(0, format!("Reading DVS file ..."));
	let mut cfbfdata: Vec<u8> = Vec::new();
	DvsFile::new(input, debug)?.decompress(&mut cfbfdata, debug)?;
	debug.logln(0, format!("Read DVS file."));

	// Parse CFBF file
	debug.logln(0, format!("Reading CFBF file ..."));
	let mut container = Container::new(SeekableRead::new(&cfbfdata[..])?, debug)?;
	debug.logln(0, format!("Read CFBF file."));

	// Open the target file for writing
	let mut target_file: Box<Write> = match target_dir {
		"" => Box::new(stdout()),
		_ => {
			// In the CFBF file, there are two embedded files that hold the original file path:
			// (1) '/User Information/Location/ExchangeLocation/FolderPath' for the directory name
			// (2) '/User Information/User Archivable Item/Title' for the file name
			let original_path_dir = read_path_from_embedded_file(&mut container, &["Root Entry".to_owned(), "User Information".to_owned(), "Location".to_owned(), "ExchangeLocation".to_owned(), "FolderPath".to_owned()], false, true, debug)?;
			debug.logln(0, format!("Original directory: {:?}", original_path_dir));
			let original_path_file = read_path_from_embedded_file(&mut container, &["Root Entry".to_owned(), "User Information".to_owned(), "User Archivable Item".to_owned(), "Title".to_owned()], true, true, debug)?;
			debug.logln(0, format!("Original file name: {:?}", original_path_file));

			// Make sure that the destination directory exists, and also use the canonicalized version of the path
			// Under Windows, this uses UNC paths that support long file names
			let mut target_path = PathBuf::from(target_dir);
			create_dir_all(&target_path)?;
			target_path = target_path.canonicalize()?;

			// Create the sub-folder that will contain the to-be-created file in the target directory,
			// with the relative path specified by 'original_path_dir/original_path_file'
			target_path.push(&original_path_dir);
			debug.logln(0, format!("Create directory (if it does not exist): {:?}", target_path));
			create_dir_all(&target_path)?;

			// Deduce the path of the target file
			target_path.push(&original_path_file);

			// Open the file for writing
			debug.log(0, format!("Creating file {:?} ... ", target_path));
			let target_file = Box::new(File::create(target_path)?);
			debug.logln(0, format!("OK."));
			target_file
		},
	};

	// First look for an outsourced file
	match process_dump_outsourced(inputfile_outsourced, &mut target_file, debug) {
		Ok(true) => Ok(()),
		Ok(false) => {
			// If there is no outsourced file, look for an embedded file
			match process_dump_object(&mut container, &mut target_file, debug) {
				Ok(true) => Ok(()),
				Ok(false) => {
					Err(Error::new(ErrorKind::InvalidData, "Unable to find embedded file '/**/FileContentStream', and no outsourced file can be found!"))
				},
				Err(e) => Err(e),
			}
		},
		Err(e) => Err(e),
	}
}

fn process_dump_object(container: &mut Container<SeekableRead>, target_file: &mut Box<Write>, debug: &mut Debug) -> Result<bool, Error> {
	// Find the object that contains the archived data
	// In the CFBF file, this is the embedded file '/Sharable Content/Archivable Item/FileContentStream'
	// But let's be more generous and search for any 'FileContentStream' in the entire file
	debug.logln(0, format!("Locating archived data in the CFBF file ..."));
	let archived_data_object = container.find_child_by_name("FileContentStream", debug)?;

	match archived_data_object {
		// If there is an embedded file 'FileContentStream', just copy it to the output
		ObjectResult::Ok(object) => {
			debug.logln(0, format!("Found '/**/FileContentStream'."));
			debug.logln(0, format!("Dumping archived data ..."));
			container.dump_stream(&object, target_file, debug)?;
			debug.logln(0, format!("Done."));
			Ok(true)
		},
		// If there is no embedded file 'FileContentStream',
		// return 'false' to indicate that no object has been found
		ObjectResult::None => {
			debug.logln(0, format!("The embedded file '/**/FileContentStream' does not exist in this file!"));
			Ok(false)
		}
	}
}

fn process_dump_outsourced(inputfile_outsourced: Option<String>, target_file: &mut Write, debug: &mut Debug) -> Result<bool, Error> {
	match inputfile_outsourced {
		Option::None => {
			debug.logln(0, format!("No outsourced file can be deduced (reading from STDIN)! Skipping."));
			Ok(false)
		},
		Option::Some(inputfile_outsourced) => {
			debug.log(0, format!("Looking for outsourced file '{}' ... ", inputfile_outsourced));
			match PathBuf::from(inputfile_outsourced).canonicalize() {
				// If the outsourced file exists, copy its contents to the target file
				Ok(inputfile_outsourced) => {
					debug.logln(0, format!("OK."));

					debug.log(0, format!("Opening '{:?}' for reading ...", inputfile_outsourced));
					let mut inputfile_outsourced = File::open(inputfile_outsourced)?;
					debug.logln(0, format!("Done."));

					debug.log(0, format!("Copying outsourced file ..."));
					copy(&mut inputfile_outsourced, target_file)?;
					debug.logln(0, format!("Done."));
					Ok(true)
				}
				// If the outsourced file does not exist,
				// return 'false' to indicate that the outsourced file has not been found
				Err(_) => {
					debug.logln(0, format!("not found!"));
					Ok(false)
				}
			}
		}
	}
}

/// Reads an embedded file and interpretes it as a UTF-16 string (prefixed by a byte length) encoding a path.
/// The `as_single_component` argument specifies whether all directory separators shall be escaped (`true`) or not (`false`).
fn read_path_from_embedded_file<TFile>(container: &mut Container<TFile>, path: &[String], as_single_component: bool, strip_root: bool, debug: &mut Debug) -> Result<PathBuf, Error> where TFile: Read + Seek {
	debug.logln(0, format!("Reading content of {} ...", path.join("/")));
	//match container.find_child_by_path(path, debug)? {
	match container.find_child_by_name(&path.last().unwrap(), debug)? {
		ObjectResult::None => {
			debug.logln(0, format!("Embedded file {} could not be found!", path.join("/")));
			return Err(Error::new(ErrorKind::InvalidData, "Unable to find an embedded file!"));
		},
		ObjectResult::Ok(object) => {
			// Read the content from that embedded file
			debug.logln(0, format!("Reading embedded file {} ...", path.join("/")));
			let mut buffer: Vec<u8> = Vec::new();
			container.dump_stream(&object, &mut buffer, debug)?;
			debug.logln(0, format!("Read {} bytes.", buffer.len()));

			// The file content is a 32bit integer denoting the byte length, followed by a NUL-terminated UTF16-encoded string
			// Extract the Unicode code points (as u16's); also remove the trailing NUL character
			let mut buffer16 = vec![0u16; (buffer.len() - 4) / 2 - 1];
			for i in 0..buffer16.len() {
				buffer16[i] = (buffer[i * 2 + 4] as u16) | (buffer[i * 2 + 5] as u16) << 8;
			}

			// Convert it into a string and then into a (possibly absolute) path
			debug.logln(0, format!("Interpreting file content as string ..."));
			let result = PathBuf::from(decode_utf16(buffer16.iter().cloned())
				.map(|c| c.unwrap_or(REPLACEMENT_CHARACTER))
				.map(|c| if c == '\x7F' || c == '\\' || c == '/' { if as_single_component { '_' } else { '/' } } else { c })
				.collect::<String>());

			// Make sure that the path is not absolute by removing prefixes and root directories
			Ok(result
				.components()
				.filter(|c| match c { Component::Prefix(_) | Component::RootDir => !strip_root, _ => true })
				.collect::<PathBuf>())
		},
	}
}