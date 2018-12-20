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

use std::io::{Read, Seek, SeekFrom, Write};
use std::io::Error;
use std::io::Stderr;

pub struct SeekableRead {
	data: Vec<u8>,
	index: u64,
}

impl SeekableRead {
	pub fn new(mut read: impl Read) -> Result<SeekableRead, Error> {
		let mut data: Vec<u8> = Vec::new();
		read.read_to_end(&mut data)?;
		Ok(SeekableRead { data, index: 0 })
	}
}

impl Read for SeekableRead {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
		(&self.data[self.index as usize..]).read(buf)
	}
}

impl Seek for SeekableRead {
	fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error> {
		match pos {
			SeekFrom::Start(i) => self.index = i,
			SeekFrom::Current(i) => self.index = (self.index as i64 + i) as u64,
			SeekFrom::End(i) => self.index = (self.data.len() as i64 + i) as u64,
		}
		Ok(self.index)
	}
}

pub struct Debug {
	output: Stderr,
	level: i8,
}

impl Debug {
	pub fn new(output: Stderr, level: i8) -> Debug {
		Debug { output, level }
	}

	pub fn log(&mut self, level: i8, string: String) {
		if self.level >= level {
			self.output.write_all(string.as_bytes()).unwrap();
		}
	}

	pub fn logln(&mut self, level: i8, string: String) {
		if self.level >= level {
			self.output.write_all(string.as_bytes()).unwrap();
			self.output.write_all(&[0x0A]).unwrap();
		}
	}
}
