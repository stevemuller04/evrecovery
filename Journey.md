# My journey through recovering files from a faulty Enterprise Vault installation

## Background

At my new working place, one of the first tasks I was given was to recover files that were lost during a migration of a proprietary archiving solution.
In fact I was given the task to find a service provider who knows the software good enough to be able to recover from a failed migration.

I was very suspicious about being successful in that direction. I thus decided to investigate a bit further into what could have gone wrong, so that I could narrow down my search for a competent service provider, or at least give them more specific information.

My first investigations revealed that the files were not technically lost: the archives were still there, and were also backed up.
However, as it happens, during the migration some files were not properly taken over to the new installation (supposedly due to very unorthodox file permissions set by the file owners).
Unfortunately, this was the case only for ‟a handful* of files” and thus remained unnoticed at first. (* That was what I was told – but as I found out later, more than 105,000 files were affected!)
The migration was finalised and the database upgraded.

A few months later, some people realised that not all files had been recovered. In fact, the files were _there_, but no one could open them. Not even system administrators, who had all permissions. Ouch. They tried to roll back to the old version of the archiving software that still had all the files, but the database was upgraded in the mean time, so the old version was no longer compatible with the database structure. So getting the files from here would not work, either.
Last resort – recovery tools. In fact, the archiving solution _does_ provide quite some recovery tools, but they work by sending requests to the main service of the archiving software – which refuses to start up.

So in the end they remained with two versions of the software: an old one that has all the files, but which cannot be launched (and for which the recovery tools thus do not work); and a new one, which can be launched, but which is missing a decent portion of the files.

## Functioning

Enterprise Vault is based on a very simple principle: files are stored on a normal file server. When a file is considered being unused, it is archived and copied to an archiving server in the backend. The actual file on the file server is replaced by a place holder that does not consume any disk space. When a user tries to access this place holder, the software automatically retrieves the file from the archives, and transparently replaces the place holder by this file.

To achieve this, an Enterprise Vault agent/driver must be installed on the system. There is no documentation on how it works, though.

## Locating the data

During my investigations, my first goal was to actually _locate_ the data: there are several hard disks and an enormous database, but where are the files actually stored? Even after searching the entire database table by table, I was unable to retrieve even the original file names or their paths. I was confused: how does the archiving software do the matching between originals and archives?
I still have not figured that out, but my gut feeling tells me that it documents’t. The reparse point contains all necessary information (ID of the archive, among others) to retrieve the file, so that the back-end does not need to care about that.

So the database did not reveal too much information. I started looking at the disks. Some of the disks are called `Vault Partition`. Hmm, promising.
Each of them contains a folder `Enterprise Vault Stores`, which in return contains two folders `ExchVStore Ptn0` and `FSAVStore Ptn0`.
From what I read on forums (which i very little), Enterprise Vault also supports archiving Exchange folders, so the first one probably contains Exchange data, while the second contains actual files (my guess is FSA = file system archive).
From the size of these folders (roughly 400 MiB each) I deduced that these partitions actually contain all the archived data. Unfortunately, all what they contain are mysterious `.dvs` files (sometimes packed into a `.cab` archive).

## Decoding the data

My first guess was that they are encrypted versions of the files. A quick binary analysis yielded a more or less uniform distribution, which also made me think that they are indeed encrypted.
However, when I tried to figure out what encryption algorithm they use, and where the keys are possibly stored, I stumbled upon a post in the official support forum, stating that Enterprise does not use encryption at all. Oh. I had a closer look at the file:

```
Offset(h) 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F

00000000  FF EE EE DD 00 00 00 00 01 00 00 00 00 00 00 00  ÿîîÝ............
00000010  0D 00 32 00 00 05 0C 00 00 01 00 00 00 78 9C ED  ..2..........xœí
00000020  5A 4D 6C 1B C7 15 1E C9 7F B2 63 CB 96 7F 14 47  ZMl.Ç..É.²cË–..G
00000030  51 14 86 FE 89 63 5B 12 FF 49 C9 14 E5 E5 92 6B  Q.†þ‰c[.ÿIÉ.åå’k
00000040  3B 91 62 55 94 1D DB 51 EC 52 12 6D A9 16 29 41  ;‘bU”.ÛQìR.m©.)A
00000050  A4 15 27 41 6B C4 E9 21 40 92 A2 08 10 20 08 9A  ¤.'AkÄé!@’¢.. .š
00000060  43 02 04 E8 A5 07 DF DC 43 52 B4 97 16 2D 8A 34  C..è¥.ßÜCR´—.-Š4
00000070  E9 A1 87 16 A8 8B F4 D4 1F 17 E8 A1 35 0A C4 EA  é¡‡.¨‹ôÔ..è¡5.Äê
00000080  F7 DE CE 88 CB 95 28 ED 6E 85 06 2D 3C C4 C7 9D  ÷ÞÎˆË•(ín….-<ÄÇ.
00000090  9D 9D 9D F7 66 DE 9B F7 DE CC EC AF 3E 6D B9 F3  ...÷fÞ›÷ÞÌì¯>m¹ó
          ...
```

It starts with a header, okay.
My first instinct is always to look for payload lengths. In this case, the file has a total length of 3102 bytes (hexadecimal 0xC1E).
Does anything in there look similar? Well, yes, there is `05 0C 00 00` representing 0xC05. And indeed, it is followed by exactly 3077 (0xC05) bytes.
In comparison to other files – yes, this seems to work out. And that length is always at the same place in the file, so it is probably part of the file header. Good.

Now let’s try to figure out what’s behind. For all the files I looked at, the payload length is followed by `01 00 00 00`. Is it some kind of ID?
And then the sequence `78 9C ED` – but hey, I recognise this one! (Actually, I looked it up. I don’t know all magic numbers by heart.)
`78 9C ED` is one of the possible headers of the zlib compression library. Compression? Ah, indeed, encryption algorithms are not the only ones whose output looks pretty random!

But this is great! Does that mean that the archive file is just a zlib-compressed version of the original file?

```
Offset(h) 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F

00000000  D0 CF 11 E0 A1 B1 1A E1 00 00 00 00 00 00 00 00  ÐÏ.à¡±.á........
00000010  00 00 00 00 00 00 00 00 3E 00 03 00 FE FF 09 00  ........>...þÿ..
00000020  06 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00  ................
00000030  01 00 00 00 00 00 00 00 00 10 00 00 02 00 00 00  ................
00000040  01 00 00 00 FE FF FF FF 00 00 00 00 00 00 00 00  ....þÿÿÿ........
00000050  FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF  ÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿ
00000060  FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF  ÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿ
00000070  FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF  ÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿ
00000080  FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF  ÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿÿ
          ...
```

From the magic number, this could be a Word document (the old format, before Microsoft switched to OOXML (.docx))!
Nice, because that could actually be; a lot of the files on the file share are actually Word documents.
Unfortunately, Word cannot open them. The file is damaged, apparently.

This lead me to an entire research on the format that Word uses. In fact, Microsoft developped a file format that mimicks an entire file system (very much like FAT), for storing a file hierarchy in a single file. They call it Compound File Binary File (CFBF). It is a very old format, which is no longer used – fortunately, they have a lot of documentation on it (https://msdn.microsoft.com/en-us/library/dd942138.aspx).

The CFBF files produced by Enterprise Vault always have the same embedded file hierarchy:

```
Root Entry/
├─ Archivable Item/
│  └─ FileContentStream
├─ Indexable Item/
│  ├─ Indexable Item Properties
│  └─ Indexable Item Stream
├─ LargeFile/
│  └─ LargeFileSize
├─ User Information/
│  └─ 00000000000000000000000000000000/
│     ├─ Location/
│     │  ├─ ExchangeLocation/
│     │  │  ├─ FolderEntryId
│     │  │  ├─ FolderPath
│     │  │  ├─ Machine
│     │  │  ├─ MsgStoreEntryId
│     │  │  └─ Volume
│     │  └─ FileSystemLocation/
│     │     └─ FilePath
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
│     │  ├─ OriginalSize
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

And hey, the embedded file `FileContentStream` contains exactly the original file content, byte by byte.
What’s even better, the `FolderPath` and `Title` files contain the original storage location (folder) and the original name of the file!
That means, I cannot only restore the files, I can also restore them to the correct location. Excellent!

I then spent several days to write a tool that automatically decodes the data contains in a `.dvs` file and restores it to a recovery directory, respecting the original file hierarchy.
The tool is licensed under a GPL 3.0 open-source license, which allows everyone to use and adapt it according to his needs (even though the command line parameters give you quite some flexibility already). I opted for a copy-left license to ensure that the tool will remain open and will not be made part of a proprietary solution.

**Note:** Sometimes (apparently when the original file is large), the file will not be archived within the `.dvs` file.
Instead, it will be out-sourced to an external `.dvf` file that has the same name (except for the extension), and which lives in one of the sub-folders of the directory where the `.dvs` file is located. This `.dvf` file is exactly the original file, byte by byte. One still needs to read the `.dvs` file in order to find out the original path and file name.

## Understanding place holders

Phew, the most complicated part is done. Now, recovering should be easy. Should it?

As it turns out, just copying the files from the recovery destination back to the live file server is not straight-forward. Even though I am system administrator and should have all access rights to the file in question, I cannot delete the file. I cannot even open it. It says ‟The system cannot access the file”. Wait, what?
Enterprise Vault is no longer installed on the system, so it cannot ‛block’ or whatever it tries to do.

So it must be something that is native to the file system (NTFS, in this case).
Unfortunately, there is absolutely no documentation on this. I know that these files are somewhat place holders for the original files, but whenever I search for these keywords on the Internet, I only get very few results, and all of them redirect me to the product website of Enterprise Vault.
So searching does not get me anywhere.

There is, however, something that is weird. Windows Explorer seems to ‛detect’ which files are faulty, because it represents it in a slightly transparent fashion.
After some further searches, I realised that this is because of the file attribute `O` (`Offline`) that is set on all the faulty files. And indeed, when I toggle the file attribute, it is represented as a normal file. Which is still cannot open. Dead-end here, as well.

But something different raised my attention. The place holder is actually reported to have a certain file size, but a disk usage of zero bytes. In the NTFS world, this is called a sparse file (recognisable by the file attribute `P`). Hmm, a file without actual disk usage? That reminds me of symbolic links! Do they actually use symbolic links? That would explain why ‟the system cannot access the file” – the target of the link is no longer there!
Further investigation revealed that NTFS generalised the concept of symbolic links to the notion of _reparse points_. Reparse points are file system entries that needs to be re-interpreted in order to find the actual file content.
As it turns out, Enterprise Vault seems to create a custom reparse point for archived files. When a file needs to be accessed, the NTFS file system invokes the Enterprise Vault filesystem driver, which then retrieves the original file from the back-end server.

There is a native Windows utility `fsutil` that is able to read out reparse points.
This is an example of the output of `fsutil reparsepoint query <<FILE>>`:

```
Reparse Tag Value : 0x00000010
GUID : {9DD58ACD-4BE7-4F36-9CE3-B7738EE3C702}

Reparse Data Length: 0x00000840
Reparse Data:
0000:  61 1e 00 00 45 00 2d 00  56 00 41 00 55 00 4c 00  a...E.-.V.A.U.L.
0010:  54 00 30 00 31 00 32 00  33 00 34 00 35 00 36 00  T.0.1.2.3.4.5.6.
0020:  37 00 38 00 00 00 00 00  5c 41 02 00 00 00 00 00  7.8.....\A......
0030:  00 50 02 00 00 00 00 00  01 01 00 00 68 00 74 00  .P..........h.t.
0040:  74 00 70 00 3a 00 2f 00  2f 00 65 00 76 00 73 00  t.p.:././.e.v.s.
0050:  65 00 72 00 76 00 65 00  72 00 2f 00 45 00 6e 00  e.r.v.e.r./.E.n.
0060:  74 00 65 00 72 00 70 00  72 00 69 00 73 00 65 00  t.e.r.p.r.i.s.e.
0070:  56 00 61 00 75 00 6c 00  74 00 2f 00 64 00 6f 00  V.a.u.l.t./.d.o.
0080:  77 00 6e 00 6c 00 6f 00  61 00 64 00 2e 00 61 00  w.n.l.o.a.d...a.
0090:  73 00 70 00 3f 00 56 00  61 00 75 00 6c 00 74 00  s.p.?.V.a.u.l.t.
00a0:  49 00 44 00 3d 00 31 00  45 00 35 00 44 00 33 00  I.D.=.1.E.5.D.3.
00b0:  45 00 46 00 45 00 38 00  38 00 39 00 43 00 32 00  E.F.E.8.8.9.C.2.
00c0:  37 00 41 00 34 00 33 00  42 00 43 00 35 00 38 00  7.A.4.3.B.C.5.8.
00d0:  32 00 41 00 43 00 38 00  30 00 31 00 31 00 38 00  2.A.C.8.0.1.1.8.
00e0:  42 00 46 00 42 00 36 00  31 00 31 00 31 00 30 00  B.F.B.6.1.1.1.0.
       ...
```

Here we find the same ID (which is probably a hash value) that we encountered before when unpacking the `.dvs` files.
And this is also how Enterprise Vault matches an original file to an archive! Great, I got it.

## Recover

Excellent, now that I know what these place holders are, I should be able to delete them.
Fortunately, the `fsutil` also has a `delete` option to remove reparse points again. It is invoked as `fsutil reparsepoint delete <<FILE>>`.

This seems to work fine.

For most files.

From time to time, the `fsutil reparsepoint delete` command still seems to fail. It says ‟Access denied”. But what access rights should I be missing, shouldn’t I have all permissions as an administrator?
In theory, yes. In practise, some file owners apparently revoked all file permissions for everyone (even the administrators); probably for confidentiality reasons.
But this is known terrain; to regain access on these files, all I need is `takeown` and `icacls`:

```
takeown /F <<FILE>>
icacls <<FILE>> /grant administrator:F
```

Running `fsutil reparsepoint delete <<FILE>>` then works.

For most files.

Some trial and error revealed that it is not enough to have file permissions on the place holder itself, but also on the parent directory.
Running `icacls /grant` on the parent directory then did the trick.

For most files.

I double-checked all permissions, also further up the file tree (looking for `Deny` permissions, or similar). There was nothing. I should have access to the file.
Much later, I noticed by accident that `fsutil reparsepoint query <<FILE>>` works, while `delete` does not. So what could possibly block me from deleting something when I can read it?
And then, even later, when I was playing around with file attributes, I noticed that some files had the `ReadOnly` flag set. You must be kidding.
And indeed, removing the flag resulted in the desperately sought success.

For most files.
(This is no longer funny. Something is seriously wrong with this file system.)

Unfortunately, I was unable to determine why the approach failed in these exceptional cases. Given that all remaining files (and this time it was really only a handful of them) were no longer needed, I decided to just skip them and leave the place holders.

I suspect that the following circumstances could be a possible source of error for the Windows tools:

* File names containing a tilde `~` (such as Microsoft Office lock files)
* Files with very long file names

### A word on administrators and UAC

By the way, if you try to recover files as a user that is not the in-built administrator, you might notice that you do not have full administrator rights, even if you are in the local administrator group. This is due to User Access Control (UAC), which sort of blocks the membership to the `Administrator` group. To work around this, there are a few options:

* Run the program/script as an administrator (right-click file, select ‛Run as administrator’)
* Right-click the executable, go to the ‛Compatibility’ tab, and tick ‛Run as administrator’.
* Disable UAC via group policy: in the group policy editor (`gpedit`), go to `Computer settings\Windows settings\Security settings\Local policies\Security options`. Set `User Account Control: Admin Approval Mode for the built-in Administrator account` and `User Account Control: Run all administrators in Admin Approval Mode` both to `Disabled.`

## Automate

While of this works, I would not want to do it manually. As mentioned already, more than 105,000 files needed to be recovered in my case.
Here are some of the Powershell scripts I used for automatising everything (from unpacking to restoring the files back on the file server).

In our set-up:
* the file server data was stored in `F:\Data\`
* the Enterprise Vault partitions were `S:\`, `V:\` and `W:\`
* an empty disk that served as temporary recover location, was mounted at `U:\`

By the way: you should work with [UNC paths](https://en.wikipedia.org/wiki/Uniform_Naming_Convention) since they support path lengths larger than 260 characters.
Note, however, that you need at least Powershell 5.1 for that. On our server, only Powershell 4.0 was installed, so I needed to find some work-arounds.

Another point: you may want to execute all of these scripts as the local system user, since full directory traversing might not be permitted for the administrator.
To do so, download [PsExec](https://docs.microsoft.com/en-us/sysinternals/downloads/psexec) from the Sysinternals suite (only the `PsExec.exe` or `PsExec64.exe` is needed) and start any of the powershell scripts within the Windows that opens when you run `psexec -i -s powershell`. Note that copy-paste into that window might not be possible, but you can place the script into a file (say, `tmp.ps1`) and invoke it using `. .\tmp.ps1` in the shell.

**Mass-recover all DVS files**

If the `.dvs` files are located directly on your vault partition (not in `.cab` archives), this command will invoke `dvsrestore` on all of them.
In this case, files will be restored from `S:\` to `U:\S\`, and any errors written to `U:\errors-s.txt`.
Note that `dvsrestore` natively supports long paths.

```powershell
Get-ChildItem -Path 'S:\Enterprise Vault Stores\FSAVStore Ptn2' -Filter *.dvs -Recurse | Foreach-Object {
	U:\dvsrestore.exe -t U:/S/ $_.FullName;
	if ($LASTEXITCODE -ne 0) { echo $_.FullName }
} > U:/errors-s.txt
```

If the `.dvs` files are contained in intermediate `.cab` archives, the previous approach will not work.
One first needs to unpack the `.cab` files. [7-Zip](https://www.7-zip.org/) is a free utility that can do this (only `7z.exe` and `7z.dll` are needed).
The following approach will extract the `.cab` files one-at-a-time, so that the disk usage is minimal.

```powershell
Get-ChildItem -Path 'V:\Enterprise Vault Stores\FSAVStore Ptn5' -Filter *.cab -Recurse | Foreach-Object {
	$cab = $_;
	U:\7zip\7z.exe e -oU:\tmp $cab.FullName -y > $null;
	Get-ChildItem -Path $(Split-Path -parent $cab) -Recurse -Filter *.dvf | Copy-Item -Destination U:\tmp;
	Get-ChildItem -Path U:\tmp -Filter *.dvs | %{
		U:\dvsrestore.exe -t U:/V/ $_.FullName;
		if ($LASTEXITCODE -ne 0) { echo "$($_.FullName) in $($cab.FullName)" }
	};
	Remove-Item -Recurse -Path U:\tmp;
} > U:/errors-v.txt
```

**Finding all place holders on the file server**

The normal Powershell approach, assuming that the file server files are stored in `F:\Data\`:

```powershell
Get-ChildItem -Path '\\?\F:\Data' -Recurse -File | `
Where-Object {($_.attributes -band 0x1600) -eq 0x1600} | `
Select FullName | `
Out-File -width 1000 offlinefiles.txt
```

If you want to use long paths, but do not have Powershell 5.1+ installed, you can also download the [AlphaFS library](https://github.com/alphaleonis/AlphaFS/wiki/PowerShell), put the `AlphaFS.dll` in your current working directory, and load it via `Import-Module`. Then you can use this alternative script to enumerate files:

```powershell
Import-Module .\AlphaFS.dll
$dir = New-Object -TypeName Alphaleonis.Win32.Filesystem.DirectoryInfo -ArgumentList 'F:\Data'
$dir.EnumerateFiles([Alphaleonis.Win32.Filesystem.DirectoryEnumerationOptions]48) | `
Where-Object {($_.attributes -band 0x1600) -eq 0x1600} | `
Select FullName | `
Out-File -width 1000 offlinefiles.txt
```

**Recover files**

Replace all place holders (as contained in `offlinefiles.txt`) on the file server (here: `F:\Data\`) by their respective original file (here: `U:\`):

```powershell
Get-Content .\offlinefiles.txt | Foreach-Object {
	$placeholder = $_.trim();
	$original = $original.replace('F:\Data\', 'U:\');
	if (Test-Path $original) {
		takeown /F $placeholder > $null 2> $null;
		fsutil reparsepoint delete $placeholder > $null 2> $null;
		cp $original -Destination $placeholder;
		if (!$?) {
			icacls $(Split-Path -parent $placeholder) /grant administrator:F > $null 2> $null;
			icacls $placeholder /reset > $null 2> $null;
			fsutil reparsepoint delete $placeholder > $null 2> $null;
			cp $original -Destination $placeholder;
			if (!$?) {
				echo $placeholder
			}
		}
	}
} > errors.txt
```

Or do this for all files with attribute `O` (`Offline`). This basically combines the two steps ‟Finding all place holders” and ‟Recover files”.

```powershell
Import-Module .\AlphaFS.dll
$dir = New-Object -TypeName Alphaleonis.Win32.Filesystem.DirectoryInfo -ArgumentList 'F:\Data'
$dir.EnumerateFiles([Alphaleonis.Win32.Filesystem.DirectoryEnumerationOptions]48) 2> errors-enum.txt | Where-Object {($_.attributes -band 0x1600) -eq 0x1600} | Foreach-Object {
	$placeholder = $_.FullName.trim();
	$original = $placeholder.replace('F:\Data\', 'U:\');
	if ($original.startswith('U:\')) {
		takeown /F $placeholder > $null 2> $null;
		fsutil reparsepoint delete $placeholder > $null 2> $null;
		cp $original -Destination $placeholder > $null 2> $null;
		if (!$?) {
			icacls $(Split-Path -parent $placeholder) /grant administrator:F > $null 2> $null;
			icacls $placeholder /reset > $null 2> $null;
			fsutil reparsepoint delete $placeholder > $null 2> $null;
			cp $original -Destination $placeholder > $null 2> $null;
			if (!$?) {
				echo $placeholder
			}
		}
	}
} > errors.txt
```

If files must be retrieved from multiple recovery drives, this script might be useful:

```powershell
Import-Module .\AlphaFS.dll
$dir = New-Object -TypeName Alphaleonis.Win32.Filesystem.DirectoryInfo -ArgumentList 'F:\Data'
$dir.EnumerateFiles([Alphaleonis.Win32.Filesystem.DirectoryEnumerationOptions]48) 2> errors-all-enum.txt | Where-Object {($_.attributes -band 0x1600) -eq 0x1600} | Foreach-Object {
	$placeholder = $_.FullName.trim();
	$original1 = $placeholder.replace('F:\Data\', 'U:\S\');
	$original2 = $placeholder.replace('F:\Data\', 'U:\V\');
	$original3 = $placeholder.replace('F:\Data\', 'U:\W\');
	$original = '';
	if (Test-Path $original1) { $original = $original1; }
	elseif (Test-Path $original2) { $original = $original2; }
	elseif (Test-Path $original3) { $original = $original3; }
	if ($original.startswith('U:\')) {
		takeown /F $placeholder > $null 2> $null;
		fsutil reparsepoint delete $placeholder > $null 2> $null;
		cp $original -Destination $placeholder;
		if (!$?) {
			icacls $(Split-Path -parent $placeholder) /grant administrator:F > $null 2> $null;
			icacls $placeholder /reset > $null 2> $null;
			fsutil reparsepoint delete $placeholder > $null 2> $null;
			cp $original -Destination $placeholder;
			if (!$?) {
				echo "[ERROR] $placeholder"
			}
			else {
				echo "[OK_FORCED] $placeholder"
			}
		}
		else {
			echo "[OK] $placeholder"
		}
	}
	else {
		echo "[MISSING] $placeholder"
	}
} > result.log
```

**Create an index that associates archives to original files**

```powershell
Get-ChildItem -Path 'S:\Enterprise Vault Stores\FSAVStore Ptn2' -Filter *.dvs -Recurse | `
Foreach-Object { $i = $(U:\dvsrestore.exe --path-only $_.FullName); echo "$($_.FullName) -> $i" } `
> index-s.txt
```

_____
Copyright © 2018 Steve Muller.
This text is licensed under a [CC-BY 4.0 License](https://creativecommons.org/licenses/by/4.0/).
All Powershell scripts are licensed under an [MIT](https://opensource.org/licenses/MIT) license.
