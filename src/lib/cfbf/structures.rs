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

// Also see: [MS-CFB]: Compound File Binary File Format specifications, https://msdn.microsoft.com/en-us/library/dd942138.aspx

/// The header of a CFBF file, excluding the trailing DIFAT entries.
pub struct Header {
	pub signature: u64,
	pub minor_version: u16,
	pub major_version: u16,
	pub byte_order: u16,
	pub sector_shift: u16,
	pub sector_size: u64, // virtual field; not actually contained in CFBF file
	pub mini_sector_shift: u16,
	pub mini_sector_size: u64, // virtual field; not actually contained in CFBF file
	pub number_of_directory_sectors: u32,
	pub number_of_fat_sectors: u32,
	pub first_directory_sector_location: SectorLocation,
	pub mini_stream_cutoff_size: u32,
	pub first_mini_fat_sector_location: SectorLocation,
	pub number_of_mini_fat_sectors: u32,
	pub first_difat_sector_location: SectorLocation,
	pub number_of_difat_sectors: u32,
}

/// A physical sector location in a CFBF file.
pub struct SectorLocation(pub u32);
