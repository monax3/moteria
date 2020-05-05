use crate::imports::*;
const IRO_SIGNATURE: &[u8; 4] = b"IROS";
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct Entry {
    pub offset:      u64,
    pub length:      u64,
    pub name:        String,
    pub compression: Compression,
}

#[derive(Debug, Copy, Clone)]
pub enum Compression {
    None,
    LZMA,
}

impl From<u32> for Compression {
    fn from(other: u32) -> Self {
        use Compression::*;
        match other {
            0 => None,
            2 => LZMA,
            unk => unimplemented!("Unknown compression type {:x}", unk),
        }
    }
}

pub struct IRO {
    reader:    fs::File,
    cursor:    MmapCursor,
    pub files: Vec<Entry>,
}

struct MmapCursor {
    mmap:   memmap::Mmap,
    offset: Cell<usize>,
}

// impl AsRef<[u8]> for MmapCursor {
//     fn as_ref(&self) -> &[u8] {
//         &self.mmap.as_ref()[self.offset..]
//     }
// }

impl MmapCursor {
    fn as_ref<'a>(&'a self) -> &'a [u8] { &self.mmap.as_ref()[self.offset.get()..] }

    fn consume<'a, T: Sized>(&'a self) -> &'a T {
        let size = size_of::<T>();
        assert!(self.mmap.len() - self.offset.get() >= size);

        let ret = unsafe { &*(self.as_ref().as_ptr() as *const T) };
        self.skip(size);
        ret
    }

    fn skip(&self, offset: usize) { self.offset.set(self.offset.get() + offset); }

    fn set_offset(&self, offset: usize) { self.offset.set(offset); }
}

enum Version {
    V0,
    V1,
    V2,
}

#[repr(C)]
#[derive(Debug)]
struct RawHeader {
    signature:     [u8; 4],
    version:       U32<LE>,
    archive_flags: U32<LE>,
    directory:     U32<LE>,
    num_entries:   U32<LE>,
}

#[derive(Debug)]
#[repr(C)]
struct RawEntryStart {
    entry_size: U16<LE>,
    name_size:  U16<LE>,
}

#[derive(Debug)]
#[repr(C)]
struct RawEntryEnd<OFFT: fmt::Debug> {
    flags:  U32<LE>,
    offset: OFFT,
    length: U32<LE>,
}

trait AsU64: Copy {
    fn as_u64(self) -> u64;
}

impl AsU64 for U32<LE> {
    fn as_u64(self) -> u64 { self.get() as u64 }
}
impl AsU64 for U64<LE> {
    fn as_u64(self) -> u64 { self.get() }
}

fn reinterpret<'a, P: 'a + AsRef<[u8]>, T: Sized>(from: P) -> &'a T {
    let size = size_of::<T>();
    assert!(from.as_ref().len() >= size);

    unsafe { &*(from.as_ref().as_ptr() as *const T) }
}

fn reinterpret_slice<T: Sized>(from: &[u8], len: usize) -> &[T] {
    let size = size_of::<T>() * len;
    assert!(from.len() >= size);

    unsafe { std::slice::from_raw_parts(from.as_ptr() as *const T, len) }
}

pub fn open<P: AsRef<path::Path>>(path: P) -> Result<IRO> { IRO::open(path.as_ref()) }

impl IRO {
    fn open(path: &path::Path) -> Result<Self> {
        // println!("Mod: {}", path.display());
        let reader = fs::File::open(path)?;
        let mmap = unsafe { memmap::Mmap::map(&reader)? };

        // let mut base = path.file_stem().unwrap().to_string_lossy().to_string();
        // base.push(path::MAIN_SEPARATOR);
        let base = String::from("");

        let mut cursor = MmapCursor { mmap, offset: Cell::new(0) };
        let files = Self::read_header(&base, &mut cursor)?;

        Ok(Self { reader, cursor, files })
    }

    fn read_header(base: &str, mut data: &mut MmapCursor) -> Result<Vec<Entry>> {
        let num_entries;
        let version: Version;
        {
            let header: &RawHeader = data.consume();
            assert_eq!(&header.signature, IRO_SIGNATURE);
            // println!("{:?}", header);
            if header.archive_flags.get() != 0 {
                unimplemented!("Patch archive not supported");
            }
            version = match header.version.get() {
                0x10000 => Version::V0,
                0x10001 => Version::V1,
                0x10002 => Version::V2,
                unk => unimplemented!("Unsupported version {:x}", unk),
            };
            num_entries = header.num_entries.get() as usize;
        }
        let mut entries = Vec::with_capacity(num_entries);

        for _ in 0..num_entries {
            match version {
                Version::V0 => entries.push(Self::read_entry::<U32<LE>>(base, &mut data)?),
                Version::V1 | Version::V2 => entries.push(Self::read_entry::<U64<LE>>(base, &mut data)?),
            }
        }
        Ok(entries)
    }

    fn read_entry<OFFT: fmt::Debug + AsU64>(base: &str, data: &mut MmapCursor) -> Result<Entry> {
        // let mut real_buf: Vec<u8> = Vec::with_capacity(0);
        let start: &RawEntryStart = reinterpret(data.as_ref());
        let entry_size = start.entry_size.get() as usize;
        let name_size: usize = start.name_size.get() as usize;
        assert!(entry_size < 1000);
        assert_eq!(name_size % size_of::<u16>(), 0);

        let end_size = size_of::<RawEntryEnd<OFFT>>();
        // fails sometimes?
        // assert_eq!(entry_size, name_size + size_of::<RawEntryStart>() +
        // size_of::<RawEntryEnd>());
        assert!(entry_size >= name_size + size_of::<RawEntryStart>() + end_size);

        // if buf.len() < entry_size {
        //     real_buf.resize_with(entry_size, || 0);
        //     reader.read_exact(real_buf.as_mut_slice())?;
        //     buf = real_buf.as_slice();
        // }

        let buf = &data.as_ref()[..entry_size][size_of::<RawEntryStart>()..];

        // println!("buffer size: {}", buf.len());
        // println!("RawEntryStart: {}", size_of::<RawEntryStart>());
        // println!("RawEntryEnd: {}", size_of::<RawEntryEnd<OFFT>>());
        // println!("entry_size: {}", entry_size);

        let name = UStr::<u16>::from_slice(reinterpret_slice(buf, name_size / size_of::<u16>())).to_string_lossy();

        // Convert to / path separators
        #[cfg(not(windows))]
        let name: String = name.chars().map(|c| match c {
            '\\' => path::MAIN_SEPARATOR,
            c => c
        }).collect();

        // let mut name = base.to_string();
        // name.push_str(&name_str);

        let buf = &buf[name_size..];

        let end: &RawEntryEnd<OFFT> = reinterpret(buf);
        let compression = end.flags.get().into();

        let ret = Entry { name, offset: end.offset.as_u64(), length: end.length.as_u64(), compression };

        // if real_buf.is_empty() {
        data.skip(entry_size);
        // }
        Ok(ret)
    }

    fn extract_lzma<W: Write>(data: &mut MmapCursor, mut writer: W, length: u64) -> Result<()> {
        #[repr(C)]
        struct LzmaHeader {
            unpacked_size:     U32<LE>,
            properties_length: U32<LE>,
        };
        let header: &LzmaHeader = data.consume();
        assert_eq!(header.properties_length.get(), 5);

        let compressed_length = length - (size_of::<LzmaHeader>() as u64);

        // FIXME: wtf
        let uncompressed_length = header.unpacked_size.get();
        // let uncompressed_length = header.unpacked_size.get() - 1;

        let buf = &data.as_ref()[..compressed_length as usize];
        let mut cur = io::Cursor::new(buf);

        let opts =
            lzma_rs::decompress::Options { unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed_length as _)) };
        lzma_rs::lzma_decompress_with_options(&mut cur, &mut writer, &opts).map_err(|err| anyhow!("{:?}", err))?;

        Ok(())
    }

    fn extract_direct<W: Write>(data: &mut MmapCursor, mut writer: W, length: u64) -> Result<()> {
        let in_buf = &data.as_ref()[..length as usize];
        writer.write_all(&in_buf)?;

        // let mut buf: [u8; 100_000] = [0; 100000];
        // let mut remain = length as usize;
        // while remain > 0 {
        //     let buf_len = std::cmp::min(buf.len(), remain);
        //     let read = reader.read(&mut buf[..buf_len])?;

        //     let read_buf = &buf[..read];
        //     writer.write_all(&read_buf)?;
        //     remain -= read;
        // }

        Ok(())
    }

    fn extract_to_inner<W: Write>(&mut self, writer: W, length: u64, offset: u64, compression: Compression) -> Result<()> {
        self.cursor.set_offset(offset as usize);

        match compression {
            Compression::None => Self::extract_direct(&mut self.cursor, writer, length),
            Compression::LZMA => Self::extract_lzma(&mut self.cursor, writer, length),
        }
    }

    pub fn extract_to<W: Write>(&mut self, mut writer: W, entry_idx: usize) -> Result<()> {
        let Entry { length, offset, compression, .. } = &self.files[entry_idx].clone();
        self.extract_to_inner(&mut writer, *length, *offset, *compression)
    }

    pub fn extract(&mut self, entry_idx: usize) -> Result<()> {
        let Entry { length, offset, compression, name } = &self.files[entry_idx].clone();
        let path: &path::Path = name.as_ref();
        fs::create_dir_all(path.parent().unwrap())?;
        let mut writer = io::BufWriter::new(fs::File::create(path)?);
        self.extract_to_inner(&mut writer, *length, *offset, *compression)
    }

    pub fn extract_all(&mut self) -> Result<()> {
        for idx in 0..self.files.len() {
            self.extract(idx)?;
        }
        Ok(())
    }
}
