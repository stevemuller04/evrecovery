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

extern crate libflate;

use std::io::{Read, Write, Error, ErrorKind};
use std::io::copy;
use self::libflate::zlib::Decoder;
use super::io::Debug;

pub struct File<TFile> where TFile: Read {
	input: TFile,
	payload_length: u32,
	#[allow(dead_code)]
	payload_id: u32,
}

impl<TFile> File<TFile> where TFile: Read {
	pub fn new(mut input: TFile, debug: &mut Debug) -> Result<File<TFile>, Error> {
		// Read header
		debug.log(1, format!("[new] Reading file header (25 bytes) ... "));
		let mut buffer_header = [0; 25];
		input.read_exact(&mut buffer_header)?;
		debug.logln(1, format!("OK."));

		// Verify magic number
		debug.logln(1, format!("[new] Magic number is 0x{:02X}{:02X}{:02X}{:02X}.", buffer_header[0], buffer_header[1], buffer_header[2], buffer_header[3]));
		if buffer_header[0] != 0xFF || buffer_header[1] != 0xEE || buffer_header[2] != 0xEE || buffer_header[3] != 0xDD {
			debug.logln(1, format!("Bad magic number, expected 0xFFEEEEDD!"));
			return Err(Error::new(ErrorKind::InvalidData, "Bad magic number"));
		}

		// Retrieve data length
		let payload_length = (buffer_header[21] as u32) | (buffer_header[22] as u32) << 8 | (buffer_header[23] as u32) << 16 | (buffer_header[24] as u32) << 24;
		debug.logln(1, format!("[new] Length of payload is {}", payload_length));
		if payload_length < 4 {
			debug.logln(1, format!("Payload too small, expected at least 4 bytes!"));
			return Err(Error::new(ErrorKind::UnexpectedEof, "Payload length missing"));
		}

		// Read first 4 bytes of payload, which encodes some kind of ID
		debug.log(1, format!("[new] Reading payload ID ... "));
		let mut buffer_payload_id = [0; 4];
		input.read_exact(&mut buffer_payload_id)?;
		debug.logln(1, format!("OK"));

		let payload_id = (buffer_payload_id[0] as u32) | (buffer_payload_id[1] as u32) << 8 | (buffer_payload_id[2] as u32) << 16 | (buffer_payload_id[3] as u32) << 24;
		debug.logln(1, format!("[new] Payload ID is {}", payload_id));

		Ok(File { input, payload_length, payload_id })
	}

	pub fn decompress(self, output: &mut impl Write, debug: &mut Debug) -> Result<(), Error> {
		let mut input = self.input;

		// Decompress
		debug.log(1, format!("[decompress] Decompressing ... "));
		let mut payload_data = Decoder::new(input.take(self.payload_length as u64 - 4))?;
		copy(&mut payload_data, output)?;
		debug.logln(1, format!("OK."));

		// Regain ownership of input stream
		input = payload_data.into_inner().into_inner();

		// Expect EOF
		debug.log(1, format!("[decompress] Verifying if the entire file has been processed ... "));
		let mut buffer_eof = [0; 1];
		if input.read(&mut buffer_eof)? != 0 { // expect 0 bytes, i.e. expect EOF
			return Err(Error::new(ErrorKind::InvalidData, "Unexpected data after payload"));
		}
		debug.logln(1, format!("OK."));

		Ok(())
	}
}
