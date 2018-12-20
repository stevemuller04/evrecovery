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

mod structures;

use self::structures::{Header, SectorLocation};
use std::io::{Read, Seek, SeekFrom, Write, Error, ErrorKind};
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::cmp::min;
use super::io::Debug;

pub struct Container<TFile: Read + Seek> {
	file: TFile,
	header: Header,
}

#[derive(Clone)]
pub struct Object {
	pub id: u32,
	pub name: String,
	/// Whether this object is a folder or a file.
	pub object_type: ObjectType,
	/// The ID of the left sibling object in the binary tree (in this folder).
	left_sibling_id: u32,
	/// The ID of the left sibling object in the binary tree (in this folder).
	right_sibling_id: u32,
	/// If this object is a folder: the ID of the first child of this folder. Otherwise undefined.
	child_id: u32,
	pub creation_time: u64,
	pub modified_time: u64,
	/// If this object is a file: the location of the first sector that holds the file content.
	starting_sector_location: u32,
	/// If this object is a file: the length of the file content.
	stream_size: u64,
}

#[derive(Copy, Clone, PartialEq)]
pub enum ObjectType {
	/// The root folder.
	RootStorage,
	/// A folder.
	Storage,
	/// A file.
	Stream,
	Unknown,
}

#[derive(Clone)]
pub enum ObjectResult {
	Ok(Object),
	None,
}

impl<TFile> Container<TFile> where TFile: Read + Seek {
	pub fn new(mut file: TFile, debug: &mut Debug) -> Result<Container<TFile>, Error> {
		// Read header from beginning of file
		debug.log(1, format!("[new] Reading CFBF file header (76 bytes) ... "));
		let mut buffer = [0; 0x4C];
		file.seek(SeekFrom::Start(0))?;
		file.read_exact(&mut buffer)?;
		debug.logln(1, format!("OK."));

		let signature =
			(buffer[0] as u64) |
			(buffer[1] as u64) << 8 |
			(buffer[2] as u64) << 16 |
			(buffer[3] as u64) << 24 |
			(buffer[4] as u64) << 32 |
			(buffer[5] as u64) << 40 |
			(buffer[6] as u64) << 48 |
			(buffer[7] as u64) << 56;
		// skip CLSID (16 bytes)
		let minor_version = (buffer[24] as u16) | (buffer[25] as u16) << 8;
		let major_version = (buffer[26] as u16) | (buffer[27] as u16) << 8;
		let byte_order = (buffer[28] as u16) | (buffer[29] as u16) << 8;
		let sector_shift = (buffer[30] as u16) | (buffer[31] as u16) << 8;
		let mini_sector_shift = (buffer[32] as u16) | (buffer[33] as u16) << 8;
		// skip reserved (6 bytes)
		let number_of_directory_sectors = (buffer[40] as u32) | (buffer[41] as u32) << 8 | (buffer[42] as u32) << 16 | (buffer[43] as u32) << 24;
		let number_of_fat_sectors = (buffer[44] as u32) | (buffer[45] as u32) << 8 | (buffer[46] as u32) << 16 | (buffer[47] as u32) << 24;
		let first_directory_sector_location = SectorLocation((buffer[48] as u32) | (buffer[49] as u32) << 8 | (buffer[50] as u32) << 16 | (buffer[51] as u32) << 24);
		// skip transaction signature number (4 bytes)
		let mini_stream_cutoff_size = (buffer[56] as u32) | (buffer[57] as u32) << 8 | (buffer[58] as u32) << 16 | (buffer[59] as u32) << 24;
		let first_mini_fat_sector_location = SectorLocation((buffer[60] as u32) | (buffer[61] as u32) << 8 | (buffer[62] as u32) << 16 | (buffer[63] as u32) << 24);
		let number_of_mini_fat_sectors = (buffer[64] as u32) | (buffer[65] as u32) << 8 | (buffer[66] as u32) << 16 | (buffer[67] as u32) << 24;
		let first_difat_sector_location = SectorLocation((buffer[68] as u32) | (buffer[69] as u32) << 8 | (buffer[70] as u32) << 16 | (buffer[71] as u32) << 24);
		let number_of_difat_sectors = (buffer[72] as u32) | (buffer[73] as u32) << 8 | (buffer[74] as u32) << 16 | (buffer[75] as u32) << 24;

		Ok(Container {
			file,
			header: Header {
				signature,
				minor_version,
				major_version,
				byte_order,
				sector_shift,
				sector_size: 1 << sector_shift,
				mini_sector_shift,
				mini_sector_size: 1 << mini_sector_shift,
				number_of_directory_sectors,
				number_of_fat_sectors,
				first_directory_sector_location,
				mini_stream_cutoff_size,
				first_mini_fat_sector_location,
				number_of_mini_fat_sectors,
				first_difat_sector_location,
				number_of_difat_sectors,
			}
		})
	}

	pub fn get_root_object(&mut self, debug: &mut Debug) -> Result<Object, Error> {
		self.get_object(0, debug)
	}

	pub fn get_object(&mut self, id: u32, debug: &mut Debug) -> Result<Object, Error> {
		debug.logln(2, format!("[get_object] Locating object #{} ...", id));
		let sector = self.header.first_directory_sector_location.0;
		self.seek_sector_offset(sector, id as u64 * 128, debug)?; // directory entry is 128 bytes long

		debug.log(2, format!("[read_object] Reading object #{} ... ", id));
		let object = self.read_object(id);
		debug.logln(2, format!("OK."));
		object
	}

	pub fn get_first_child(&mut self, object: &Object, debug: &mut Debug) -> Result<ObjectResult, Error> {
		debug.logln(2, format!("[get_first_child] Getting first child for object #{} ...", object.id));
		if object.child_id == 0xFFFFFFFF {
			debug.logln(2, format!("[get_first_child] First child: None."));
			Ok(ObjectResult::None)
		}
		else {
			let child = self.get_object(object.child_id, debug)?;
			debug.logln(2, format!("[get_first_child] First child: #{}.", child.id));
			Ok(ObjectResult::Ok(child))
		}
	}

	pub fn get_left_sibling(&mut self, object: &Object, debug: &mut Debug) -> Result<ObjectResult, Error> {
		debug.logln(2, format!("[get_left_sibling] Getting left sibling for object #{} ...", object.id));
		if object.left_sibling_id == 0xFFFFFFFF {
			debug.logln(2, format!("[get_left_sibling] Left sibling: None."));
			Ok(ObjectResult::None)
		}
		else {
			let sibling = self.get_object(object.left_sibling_id, debug)?;
			debug.logln(2, format!("[get_left_sibling] Left sibling: #{}.", sibling.id));
			Ok(ObjectResult::Ok(sibling))
		}
	}

	pub fn get_right_sibling(&mut self, object: &Object, debug: &mut Debug) -> Result<ObjectResult, Error> {
		debug.logln(2, format!("[get_right_sibling] Getting right/right sibling for object #{} ...", object.id));
		if object.right_sibling_id == 0xFFFFFFFF {
			debug.logln(2, format!("[get_right_sibling] Right sibling: None."));
			Ok(ObjectResult::None)
		}
		else {
			let sibling = self.get_object(object.right_sibling_id, debug)?;
			debug.logln(2, format!("[get_right_sibling] Right sibling: #{}.", sibling.id));
			Ok(ObjectResult::Ok(sibling))
		}
	}

	/// Finds an object that is subordinated to the given object, by its path.
	/// The path is a collection of names for the root storage object, all intermediate storage objects (directories), and the final object (directory or file).
	/// To find an object, call this method as:
	/// ```
	/// container.find_child_by_path(&["Root Entry".to_owned(), "Dir1".to_owned(), "Dir2".to_owned(), "MyFile".to_owned()])
	/// ```
	pub fn find_child_by_path(&mut self, path: &[String], debug: &mut Debug) -> Result<ObjectResult, Error> {
		self.find_child_by_path_recursive(0, path, debug)
	}

	/// Finds an object that is subordinated to the given object, by its path.
	/// The path is a collection of names for the root storage object, all intermediate storage objects (directories), and the final object (directory or file).
	/// The `search_object_id` is the ID of the object where the recursive search shall be started.
	fn find_child_by_path_recursive(&mut self, search_object_id: u32, path: &[String], debug: &mut Debug) -> Result<ObjectResult, Error> {
		// Handle invalid cases (mostly 0xFFFFFFFE representing non-existing sibling nodes)
		if search_object_id > 0xFFFFFFFA {
			return Ok(ObjectResult::None);
		}

		let object = self.get_object(search_object_id, debug)?;
		debug.logln(2, format!("[find_child_by_path_recursive] Processing '{}' ...", object.name));

		// Recursion step: if the first path segment matches the name of the current object, proceed to the next segment
		if path[0] == object.name {
			if path.len() == 1 {
				// We found the object!
				Ok(ObjectResult::Ok(object))
			}
			else {
				debug.logln(2, format!("[find_child_by_path_recursive] Found segment '{}', locating remaining '{}' ...", object.name, path[1..].join("/")));
				self.find_child_by_path_recursive(object.child_id, &path[1..], debug)
			}
		}
		else {
			// Search left side of the binary tree
			if let ObjectResult::Ok(result) = self.find_child_by_path_recursive(object.left_sibling_id, path, debug)? {
				Ok(ObjectResult::Ok(result))
			}
			// Search right side of the binary tree
			else if let ObjectResult::Ok(result) = self.find_child_by_path_recursive(object.right_sibling_id, path, debug)? {
				Ok(ObjectResult::Ok(result))
			}
			// Nothing found in this part of the tree
			else {
				Ok(ObjectResult::None)
			}
		}
	}

	/// Finds the first object with the given name.
	pub fn find_child_by_name(&mut self, name: &str, debug: &mut Debug) -> Result<ObjectResult, Error> {
		self.find_child_by_name_recursive(0, name, debug)
	}

	/// Finds the first object with the given name.
	fn find_child_by_name_recursive(&mut self, search_object_id: u32, name: &str, debug: &mut Debug) -> Result<ObjectResult, Error> {
		// Handle invalid cases (mostly 0xFFFFFFFE representing non-existing sibling nodes)
		if search_object_id > 0xFFFFFFFA {
			return Ok(ObjectResult::None);
		}

		let object = self.get_object(search_object_id, debug)?;
		debug.logln(2, format!("[find_child_by_name_recursive] Processing '{}' ...", object.name));

		// Recursion step: if the first path segment matches the name of the current object, proceed to the next segment
		if name == object.name {
			Ok(ObjectResult::Ok(object))
		}
		else {
			// Search left side of the binary tree
			if let ObjectResult::Ok(result) = self.find_child_by_name_recursive(object.left_sibling_id, name, debug)? {
				Ok(ObjectResult::Ok(result))
			}
			// Search right side of the binary tree
			else if let ObjectResult::Ok(result) = self.find_child_by_name_recursive(object.right_sibling_id, name, debug)? {
				Ok(ObjectResult::Ok(result))
			}
			// Search child of the binary tree
			else if let ObjectResult::Ok(result) = self.find_child_by_name_recursive(object.child_id, name, debug)? {
				Ok(ObjectResult::Ok(result))
			}
			// Nothing found in this part of the tree
			else {
				Ok(ObjectResult::None)
			}
		}
	}

	pub fn dump_stream(&mut self, object: &Object, output: &mut Write, debug: &mut Debug) -> Result<(), Error> {
		debug.logln(2, format!("[dump_stream] Dumping data for stream #{} ({} bytes) ...", object.id, object.stream_size));
		// This method only makes sense for stream objects (i.e. files) and the root storage (which contains the ministream)
		match object.object_type {
			ObjectType::RootStorage => self.dump_stream_normal(object, output, debug),
			ObjectType::Stream =>
				// Two cases: if the file is small, look for it in the ministream; otherwise read it from a sector
				if object.stream_size < self.header.mini_stream_cutoff_size as u64 {
					self.dump_stream_mini(object, output, debug)
				}
				else {
					self.dump_stream_normal(object, output, debug)
				},
			_ => Err(Error::new(ErrorKind::InvalidData, "Cannot dump data of storage objects!"))
		}
	}

	fn dump_stream_mini(&mut self, object: &Object, output: &mut Write, debug: &mut Debug) -> Result<(), Error> {
		// Locate ministream
		debug.logln(2, format!("[dump_stream_mini] Locate ministream ..."));
		let ministream_starting_location = self.get_root_object(debug)?.starting_sector_location;

		// Start copying from starting mini-sector in the ministream
		debug.logln(2, format!("[dump_stream_mini] Dumping stream #{} from mini stream ({} bytes) ...", object.id, object.stream_size));
		let mut size_remaining = object.stream_size;
		let mut current_minisector = object.starting_sector_location;
		while size_remaining > 0 {
			// Don't copy more bytes than there are in this mini-sector
			let num_bytes = min(64, size_remaining);
			debug.logln(2, format!("[dump_stream_mini] Need to copy {} bytes from mini-sector #{}. Going there ...", num_bytes, current_minisector));
			self.seek_minisector_offset(ministream_starting_location, current_minisector, 0, debug)?;

			// Copy data from current mini-sector to output
			debug.log(2, format!("[dump_stream_mini] Copying ... "));
			self.write_bytes(output, num_bytes)?;
			debug.logln(2, format!("OK."));

			// If bytes are remaining, we need to find the next mini-sector
			size_remaining -= num_bytes;
			if size_remaining > 0 {
				debug.logln(2, format!("[dump_stream_mini] {} bytes remaining. Determining next mini-sector ...", size_remaining));
				current_minisector = self.get_next_minisector(current_minisector, debug)?;
			}
		}
		debug.logln(2, format!("[dump_stream_normal] Done dumping."));
		Ok(())
	}

	fn dump_stream_normal(&mut self, object: &Object, output: &mut Write, debug: &mut Debug) -> Result<(), Error> {
		debug.logln(2, format!("[dump_stream_normal] Dumping stream #{} from sectors ({} bytes) ...", object.id, object.stream_size));
		// Start copying from starting sector
		let mut size_remaining = object.stream_size;
		let mut current_sector = object.starting_sector_location;
		while size_remaining > 0 {
			// Don't copy more bytes than there are in this sector
			let num_bytes = min(self.header.sector_size, size_remaining);
			debug.logln(2, format!("[dump_stream_normal] Need to copy {} bytes from sector #{}. Going there ...", num_bytes, current_sector));
			self.seek_sector(current_sector, debug)?;

			// Copy data from current sector to output
			debug.log(2, format!("[dump_stream_normal] Copying ... "));
			self.write_bytes(output, num_bytes)?;
			debug.logln(2, format!("OK."));

			// If bytes are remaining, we need to find the next sector
			size_remaining -= num_bytes;
			if size_remaining > 0 {
				debug.logln(2, format!("[dump_stream_normal] {} bytes remaining. Determining next sector ...", size_remaining));
				current_sector = self.get_next_sector(current_sector, debug)?;
			}
		}
		debug.logln(2, format!("[dump_stream_normal] Done dumping."));
		Ok(())
	}

	/// Retrieves the sector number of the sector that follows the given sector in the chain (from the FAT).
	fn get_next_sector(&mut self, sector: u32, debug: &mut Debug) -> Result<u32, Error> {
		debug.logln(3, format!("[get_next_sector] Retrieving sector number that follows sector#{} ...", sector));

		// First identify the index of FAT sector that holds the information that we need, and the relative offset in that sector.
		// Every FAT sector contains exactly `SECTOR_SIZE / 4` entries.
		let fat_entries_per_sector = (self.header.sector_size / 4) as u32;
		let fat_sector_index = sector / fat_entries_per_sector;
		let fat_sector_offset = (sector % fat_entries_per_sector) as u64 * 4;
		debug.logln(3, format!("[get_next_sector] This should be written in {}th FAT sector at offset {:#X}. Locating that FAT sector ...", fat_sector_index, fat_sector_offset));

		// Now determine the physical location of the concerned FAT sector
		// The first 109 FAT sectors numbers are listed right after the header, whereas all subsequent ones are listed in the so-called DIFAT sectors.
		if fat_sector_index < 109 {
			debug.log(3, format!("[get_next_sector] The location of the FAT sector should be at the end of the CFBF header. Going there ... "));
			self.file.seek(SeekFrom::Start(0x4C + fat_sector_index as u64 * 4))?; // after the header
			debug.logln(3, format!("OK."));
		}
		else {
			debug.logln(3, format!("[get_next_sector] The location of the FAT sector is in the DIFAT, not at the end of the CFBF header."));
			let mut relative_fat_sector_index = fat_sector_index - 109;

			// Move to first DIFAT entry
			debug.logln(3, format!("[get_next_sector] First DIFAT sector is at {:#X}. Going there ...", self.header.first_difat_sector_location.0));
			let first_difat_sector_location = self.header.first_difat_sector_location.0;
			self.seek_sector(first_difat_sector_location, debug)?;

			// Move to the DIFAT sector that holds the location of the FAT sector we are looking for
			let difat_entries_per_sector = (self.header.sector_size / 4 - 1) as u32;
			while relative_fat_sector_index >= difat_entries_per_sector {
				debug.logln(3, format!("[get_next_sector] The location of the FAT sector is not in this DIFAT sector."));

				// Retrieve location of next DIFAT sector
				debug.log(3, format!("[get_next_sector] Reading location of next DIFAT sector ... "));
				self.file.seek(SeekFrom::Current(difat_entries_per_sector as i64 * 4))?;
				let next_difat_location = self.read_u32()?;
				debug.logln(3, format!("OK ({:#X}).", next_difat_location));

				// Go and continue looking there
				relative_fat_sector_index -= difat_entries_per_sector;
				self.seek_sector(next_difat_location, debug)?;
			}

			debug.logln(3, format!("[get_next_sector] We should be in the correct DIFAT sector now."));

			// Read the FAT sector location from here
			debug.log(3, format!("[get_next_sector] Going to the where the FAT sector location is stored ... "));
			self.file.seek(SeekFrom::Current(relative_fat_sector_index as i64 * 4))?;
			debug.logln(3, format!("OK."));
		};

		// Read the location of the FAT sector from the DIFAT
		debug.log(3, format!("[get_next_sector] Reading location of FAT sector ... "));
		let fat_sector_location = self.read_u32()?;
		debug.logln(3, format!("OK ({:#X}).", fat_sector_location));

		// We found the FAT sector (finally). Now read the entry
		debug.logln(3, format!("[get_next_sector] Going to where the next sector number is stored ... "));
		self.seek_sector_offset(fat_sector_location, fat_sector_offset, debug)?;
		debug.log(3, format!("[get_next_sector] Reading next sector number ... "));
		let result = self.read_u32()?;
		debug.logln(3, format!("OK ({}).", result));
		Ok(result)
	}

	/// Retrieves the mini-sector number of the mini-sector that follows the given mini-sector in the ministream chain (from the miniFAT).
	fn get_next_minisector(&mut self, minisector: u32, debug: &mut Debug) -> Result<u32, Error> {
		debug.logln(3, format!("[get_next_minisector] Retrieving mini-sector number that follows mini-sector#{} ...", minisector));

		// First identify the index of miniFAT sector that holds the information that we need, and the relative offset in that sector.
		// Every miniFAT sector contains exactly `SECTOR_SIZE / 4` entries.
		let minifat_entries_per_sector = (self.header.sector_size / 4) as u32;
		let minifat_sector_index = minisector / minifat_entries_per_sector;
		let minifat_sector_offset = (minisector % minifat_entries_per_sector) as u64 * 4;
		debug.logln(3, format!("[get_next_minisector] This should be written in {}th miniFAT sector at offset {:#X}. Locating that miniFAT sector ...", minifat_sector_index, minifat_sector_offset));

		// Now determine the physical location of the concerned miniFAT sector
		// The first miniFAT sector is given in the header
		let mut minifat_sector_location = self.header.first_mini_fat_sector_location.0;
		let mut jumps_remaining = minifat_sector_index;
		debug.logln(3, format!("[get_next_minisector] {}th miniFAT sector is at {:#X}.", 0, minifat_sector_location));
		while jumps_remaining > 0 {
			minifat_sector_location = self.get_next_sector(minifat_sector_location, debug)?;
			jumps_remaining -= 1;
			debug.logln(3, format!("[get_next_minisector] {}th miniFAT sector is at {:#X}.", minifat_sector_index - jumps_remaining, minifat_sector_location));
		}

		// We found the miniFAT sector. Now read the entry.
		debug.logln(3, format!("[get_next_minisector] Going to where the next mini-sector number is stored ... "));
		self.seek_sector_offset(minifat_sector_location, minifat_sector_offset, debug)?;
		debug.log(3, format!("[get_next_minisector] Reading next mini-sector number ... "));
		let result = self.read_u32()?;
		debug.logln(3, format!("OK ({}).", result));
		Ok(result)
	}

	/// Short-hand method for `seek_sector_offset(sector, 0)`.
	fn seek_sector(&mut self, sector: u32, debug: &mut Debug) -> Result<(), Error> {
		self.seek_sector_offset(sector, 0, debug)
	}

	/// Moves the file pointer to the given offset with respect to the given sector, but respecting the sector chain.
	/// If `offset` is less than the sector size, then the file pointer is moved to `sector_location + offset`.
	/// Otherwise, this method identifies the concerned sector (using the FAT) and moves the file pointer to the correct location in one of the following sectors.
	fn seek_sector_offset(&mut self, initial_sector: u32, offset: u64, debug: &mut Debug) -> Result<(), Error> {
		debug.logln(3, format!("[seek_sector_offset] Seeking to sector#{}, relative offset {:#X} ...", initial_sector, offset));
		let mut current_sector = initial_sector;
		let mut current_offset = offset;
		while current_offset >= self.header.sector_size {
			debug.logln(3, format!("[seek_sector_offset] Offset {:#X} is not in sector#{}, determining next sector ...", current_offset, current_sector));
			current_sector = self.get_next_sector(current_sector, debug)?;
			current_offset -= self.header.sector_size;
		}

		debug.log(3, format!("[seek_sector_offset] Going to sector#{}, relative offset {:#X} ... ", current_sector, current_offset));
		self.file.seek(SeekFrom::Start((current_sector as u64 + 1) * self.header.sector_size + current_offset as u64))?;
		debug.logln(3, format!("OK."));
		Ok(())
	}

	/// Moves the file pointer to the given offset with respect to the given mini-sector, but respecting the mini-sector chain.
	/// If `offset` is less than the mini-sector size, then the file pointer is moved to `minisector_location + offset`.
	/// Otherwise, this method identifies the concerned mini-sector (using the miniFAT) and moves the file pointer to the correct location in one of the following mini-sectors.
	fn seek_minisector_offset(&mut self, ministream_sector: u32, initial_minisector: u32, offset: u64, debug: &mut Debug) -> Result<(), Error> {
		debug.logln(3, format!("[seek_minisector_offset] Seeking to mini-sector#{}, relative offset {:#X} ...", initial_minisector, offset));
		let mut current_minisector = initial_minisector;
		let mut current_offset = offset;
		while current_offset >= 64 {
			debug.logln(3, format!("[seek_minisector_offset] Offset {:#X} is not in mini-sector#{}, determining next mini-sector ...", current_offset, current_minisector));
			current_minisector = self.get_next_minisector(current_minisector, debug)?;
			current_offset -= self.header.sector_size;
		}

		debug.logln(3, format!("[seek_minisector_offset] Going to mini-sector#{}, relative offset {:#X} ...", current_minisector, current_offset));
		self.seek_sector_offset(ministream_sector, current_minisector as u64 * 64 + current_offset, debug)?;
		Ok(())
	}

	fn read_u32(&mut self) -> Result<u32, Error> {
		let mut buffer = [0; 4];
		self.file.read_exact(&mut buffer)?;
		Ok((buffer[0] as u32) | (buffer[1] as u32) << 8 | (buffer[2] as u32) << 16 | (buffer[3] as u32) << 24)
	}

	fn read_object(&mut self, id: u32) -> Result<Object, Error> {
		let mut buffer = [0; 0x80];
		self.file.read_exact(&mut buffer)?;

		// Read object properties
		let mut directory_entry_name = [0u16; 32];
		for i in 0..32 {
			directory_entry_name[i] = (buffer[i * 2] as u16) | (buffer[i * 2 + 1] as u16) << 8;
		}
		let directory_entry_name_length = (buffer[64] as u16) | (buffer[65] as u16) << 8;
		let object_type = buffer[66];
		// skip color flag (1 byte)
		let left_sibling_id = (buffer[68] as u32) | (buffer[69] as u32) << 8 | (buffer[70] as u32) << 16 | (buffer[71] as u32) << 24;
		let right_sibling_id = (buffer[72] as u32) | (buffer[73] as u32) << 8 | (buffer[74] as u32) << 16 | (buffer[75] as u32) << 24;
		let child_id = (buffer[76] as u32) | (buffer[77] as u32) << 8 | (buffer[78] as u32) << 16 | (buffer[79] as u32) << 24;
		// skip CLSID (16 bytes)
		// skip state bits (4 bytes)
		let creation_time = (buffer[100] as u64) | (buffer[101] as u64) << 8 | (buffer[102] as u64) << 16 | (buffer[103] as u64) << 24 | (buffer[104] as u64) << 32 | (buffer[105] as u64) << 40 | (buffer[106] as u64) << 48 | (buffer[107] as u64) << 56;
		let modified_time = (buffer[108] as u64) | (buffer[109] as u64) << 8 | (buffer[110] as u64) << 16 | (buffer[111] as u64) << 24 | (buffer[112] as u64) << 32 | (buffer[113] as u64) << 40 | (buffer[114] as u64) << 48 | (buffer[115] as u64) << 56;
		let starting_sector_location = (buffer[116] as u32) | (buffer[117] as u32) << 8 | (buffer[118] as u32) << 16 | (buffer[119] as u32) << 24;
		let stream_size = (buffer[120] as u64) | (buffer[121] as u64) << 8 | (buffer[122] as u64) << 16 | (buffer[123] as u64) << 24 | (buffer[124] as u64) << 32 | (buffer[125] as u64) << 40 | (buffer[126] as u64) << 48 | (buffer[127] as u64) << 56;

		// Convert directory entry name to string
		let directory_entry_name = decode_utf16(
				directory_entry_name
				.iter()
				// The length is expressed in bytes, but we read u16's; also remove the trailing NUL byte
				.take((directory_entry_name_length / 2 - 1) as usize)
				.cloned())
			.map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
			.collect::<String>();

		Ok(Object{
			id,
			name: directory_entry_name,
			object_type: match object_type {
				1 => ObjectType::Storage,
				2 => ObjectType::Stream,
				5 => ObjectType::RootStorage,
				_ => ObjectType::Unknown,
			},
			left_sibling_id,
			right_sibling_id,
			child_id,
			creation_time,
			modified_time,
			starting_sector_location,
			stream_size,
		})
	}

	/// Copies the given number of raw bytes from the current position of the internal file to the given output.
	fn write_bytes(&mut self, output: &mut Write, num_bytes: u64) -> Result<(), Error> {
		let mut buffer = [0; 512];
		let mut bytes_remaining = num_bytes;
		while bytes_remaining > 0 {
			let want_read = min(bytes_remaining, 512) as usize;
			let have_read = self.file.read(&mut buffer[0..want_read])?;
			if have_read > 0 {
				output.write_all(&buffer[0..have_read])?;
				bytes_remaining -= have_read as u64;
			}
			else {
				return Err(Error::new(ErrorKind::UnexpectedEof, "Asked to copy more bytes than there are"));
			}
		}
		Ok(())
	}
}
