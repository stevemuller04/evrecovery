# Enterprise Vault Recovery toolset

This repository provides tools to recover data from a (faulty) *Enterprise Vault* instance.
All files archived by Enterprise Vault are stored on one or more physical disks in a proprietary format (`*.dvs` files).
The tools contained in this repository help in retrieving the original files.

If you are interested in the details of the file format, or techniques for mass-recovering files, I recommend you reading [my journey through recovering files from a faulty Enterprise Vault installation](Journey.md).

## dvsextract

`dvsextract` is a simple command line utility that extracts the uncompressed payload (which is supposedly in CFBF format) from a DVS file.

The utility can be invoked in two ways:
```bash
dvsextract --input $DVSFILE --output $PAYLOADFILE
dvsextract < $DVSFILE > $PAYLOADFILE
```

**Remarks:**
If the input file is missing, the utility reads from stdin instead.
If the output file is missing, the utility writes to stdout instead.
If `-v`, `-vv` or `-vvv` is specified, debug information is written to stderr.

More information can be obtained by running `dvsextract --help`.

## cfbfdump

`cfbfdump` is an analysis tool for CFBF files (including `.doc`, `.xls` and `.ppt` files).
It can list the contents of a CFBF file, and dump individual embedded files.

To list the embedded files contained in a CFBF file, run:
```bash
cfbfdump list --input $CFBFFILE
```
It will output one line per embedded file. Each line consists of the internal identifier and the path of the embedded file, separated by a space.

To dump an embedded file, run:
```bash
cfbfdump dump --id $INTERNALFILEID --input $CFBFFILE --output $OUTPUT
```

where `$INTERNALFILEID` is the internal identifier of the embedded file that was output by `cfbfdump list`.

**Remarks:**
If the input file is missing, the utility reads from stdin instead.
If the output file is missing, the utility writes to stdout instead.
If `-v`, `-vv` or `-vvv` is specified, debug information is written to stderr.

## dvsrestore

`dvsrestore` combines the functionality of `dvsextract` and `cfbfdump`.
It proceeds by:
* extracting the CFBF file from the DVS file;
* locating the embedded file `/User Information/Location/ExchangeLocation/FolderPath` and reading its contents (the directory of the original file);
* locating the embedded file `/User Information/User Archivable Item/Title` and reading its contents (the name of the original file);
* extracting the embedded file `/Sharable Content/Archivable Item/FileContentStream` (containing the original file);
* writing the latter embedded file to an actual file with the same original path, relative a given target directory.
All of this can be also achieved with `dvsextract` and `cfbfdump`, but this is a manual and slow process.

Run `dvsrestore` as:
```bash
dvsrestore -t $TARGETDIR $DVSFILE
```

Note that `dvsrestore` will try to restore out-sourced archives if the `FileContentStream` embedded file does not exist.
See the paragraph "Embedded vs. out-sourced files" below for explanations.
Starting from v1.1.0, it will first look for out-sourced archives, and only proceeds to extracting embedded files if no out-sourced archive exists.
The input file must be specified as a path, though; it will not work when reading from STDIN.
The out-sourced file path is constructed as `<input file path without extension>.dvf`. The extension (`dvf` in this case) can be customised with the `--ext` flag.

**Remarks:**
If the input file is missing, the utility reads from stdin instead.
If the target directory is missing, the utility directly outputs the original file to stdout.
If `-v`, `-vv`, `-vvv` or `-vvvv` is specified, debug information is written to stderr.

## How to compile

The tools are written in [Rust](https://www.rust-lang.org) and require the Rust compiler and Cargo to be installed.

They can be compiled as follows:

```bash
cargo build --release
```

## Mass recovery

The recommended way to use this toolset for mass recovery of files is by invoking it on all `*.dvs` files.

For example, the following Powershell script recovers all DVS files on drive `D:` to an empty drive `R:`.
If errors occurred, the names of the respective faulty DVS files are written to `errors.txt`.

```powershell
Get-ChildItem -Path D:\ -Filter *.dvs -Recurse | %{ dvsrestore.exe -t R:\ $_.FullName; if ($LASTEXITCODE -ne 0) { echo $_.FullName } } > R:\errors.txt
```

Note that the extracted files will consume a similar disk space than the DVS files (possibly even less).

# DVS file structure

A `*.dvs` file consists of a header followed by a payload.

The header is 29 bytes long and seems to be structured as follows:

* Bytes 0-3: the magic number `0xFF 0xEE 0xEE 0xDD`;
* Bytes 4-20: unknown purpose;
* Bytes 21-24: length of the payload, encoded as a 32-bit Little Endian integer;
* Bytes 25-28: seems to be always `0x01 0x00 0x00 0x00`, maybe an ID.

The payload is a zlib-compressed (starting with the 2-byte zlib header `0x78 0x9C` indicating the compression mode) version of a [Compound File Binary Format (CFBF)](https://en.wikipedia.org/wiki/Compound_File_Binary_Format) file (also known as OLE file or Structured Storage file). CFBF files imitate a file system (and are indeed inspired by the FAT file system). Even though they are mostly known for their use in `.doc`, `.ppt`, `.xls` and `.msg` (old Office format) files, the payload does not represent an actual Office document in this case (and cannot be directly opened with Word or similar, either). Instead, the payload encodes file system sectors which in turn contain the  files – one of these files is the original file that was archived; other files contain meta information such as the path of the original file.

The compressed CFBF file contains several embedded files.
A typical file structure looks like the following:

```
Root Entry/
├─ Archivable Item/
│  └─ FileContentStream (*)
├─ Indexable Item/
│  ├─ Indexable Item Properties
│  └─ Indexable Item Stream
├─ LargeFile/ (†)
│  └─ LargeFileSize (†)
├─ User Information/
│  └─ 00000000000000000000000000000000/ (‡)
│     ├─ Location/
│     │  ├─ ExchangeLocation/
│     │  │  ├─ FolderEntryId
│     │  │  ├─ FolderPath
│     │  │  ├─ Machine
│     │  │  ├─ MsgStoreEntryId
│     │  │  └─ Volume
│     │  └─ FileSystemLocation/
│     │     └─ FilePath (†)
│     ├─ Shortcut/
│     │  └─ ExchangeShortcutAccessor/
│     │     └─ ShortcutEntryId
│     ├─ User Archivable Item/
│     │  ├─ ArchivedDate
│     │  ├─ Author
│     │  ├─ CreatedTime
│     │  ├─ FileType
│     │  ├─ LastModTime
│     │  ├─ MIMEType
│     │  ├─ OriginalSize (*)
│     │  └─ Title
│     ├─ AgentIdentifier
│     ├─ AgentProperties
│     ├─ AgentQualifier
│     ├─ Retention Category
│     ├─ UserDocType
│     ├─ UserXMLStream
│     └─ VaultID
├─ Checksum
├─ CHGuid
├─ File Extension
└─ Version
```

Notes:
- `*`: Only present if the archived file is embedded in the DVS file.
- `†`: Only present if the archived file is not embedded in the DVS file, but located as a stand-alone file in the same directory as the DVS file.
- `‡`: In older versions of Enterprise Vault, this entry is a sequence of `[0-9A-F]` (probably some kind of a hash). In newer versions of Enterprise Vault, this entry is not present and all of its contents are directly located beneath `User Information/`.

**Embedded vs. out-sourced files**

Enterprise Vault seems to handle large to-be-archived files differently.
While small files are embedded directly into the DVS file (under the `/Archivable Item/FileContentStream` entry), large files are saved as individual files in the file system. They have the same name as the DVS file, but with a `.DVF` extension. The DVF file _is_ the original to-be-archived file (not compressed, not embedded).
To restore such a file, one needs to retrieve the original file name from the DVS file, and rename the DVF file accordingly.

`dvsrestore` achieves this if a file name has been specified for the input file (it will not work when reading from STDIN, for the obvious reason).

**Relevant entries**

The following entries in the CFBF file structure are relevant for extracting the original file:

- `/Archivable Item/FileContentStream`: Contains the original file
- `/User Information/[00000000000000000000000000000000/]Location/ExchangeLocation/FolderPath`: Contains the location (directory path) of the original file, encoded as a UTF-16 string preceded by a 32-bit integer denoting the string length (in bytes)
- `/User Information/[00000000000000000000000000000000/]User Archivable Item/Title`: Contains the file name of the original file, encoded as a UTF-16 string preceded by a 32-bit integer denoting the string length (in bytes)
- `/User Information/[00000000000000000000000000000000/]User Archivable Item/CreatedTime`: 32-bit integer encoding the UNIX timestamp (number of seconds since 1970-01-01 00:00:00 UTC) when the original file was created
- `/User Information/[00000000000000000000000000000000/]User Archivable Item/LastModTime`: 32-bit integer encoding the UNIX timestamp (number of seconds since 1970-01-01 00:00:00 UTC) when the original file was last modified

# License

This toolset was written by [Steve Muller](mailto:steve.muller@outlook.com) and is licensed under a GPL v3.0 License. See the [LICENSE](LICENSE) file for more details.
