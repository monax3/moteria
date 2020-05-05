use crate::imports::*;

const IRO_SIGNATURE: &[u8; 4] = b"IROS";

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
    reader:    io::BufReader<fs::File>,
    pub files: Vec<Entry>,
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

fn reinterpret<T: Sized>(from: &[u8]) -> &T {
    let size = size_of::<T>();
    assert!(from.len() >= size);

    unsafe { &*(from.as_ptr() as *const T) }
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
        let file = fs::File::open(path)?;
        let mut reader = io::BufReader::with_capacity(132768, file);

        // let mut base = path.file_stem().unwrap().to_string_lossy().to_string();
        // base.push(path::MAIN_SEPARATOR);
        let base = String::from("");

        let files = Self::read_header(&base, &mut reader)?;

        Ok(Self { reader, files })
    }

    fn read_header<R: io::BufRead + io::Seek>(base: &str, reader: &mut R) -> Result<Vec<Entry>> {
        let num_entries;
        let version: Version;
        {
            let buf = reader.fill_buf()?;
            let header: &RawHeader = reinterpret(buf);
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
        reader.consume(size_of::<RawHeader>());

        let mut entries = Vec::with_capacity(num_entries);

        for _ in 0..num_entries {
            match version {
                Version::V0 => entries.push(Self::read_entry::<_, U32<LE>>(base, reader)?),
                Version::V1 | Version::V2 => entries.push(Self::read_entry::<_, U64<LE>>(base, reader)?),
            }
        }
        Ok(entries)
    }

    fn read_entry<R: io::BufRead + io::Seek, OFFT: fmt::Debug + AsU64>(base: &str, reader: &mut R) -> Result<Entry> {
        // let mut real_buf: Vec<u8> = Vec::with_capacity(0);
        let buf = {
            let mut buf = reader.fill_buf()?;
            if buf.len() < 1000 {
                // a shameful hack to refill buffer
                reader.seek(io::SeekFrom::Current(0))?;
                buf = reader.fill_buf()?;
            }
            assert!(buf.len() >= 1000);
            buf
        };
        let start: &RawEntryStart = reinterpret(buf);
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

        assert!(entry_size <= buf.len());
        let buf = &buf[..entry_size][size_of::<RawEntryStart>()..];
        let name_str = UStr::<u16>::from_slice(reinterpret_slice(buf, name_size / size_of::<u16>())).to_string_lossy();

        let mut name = base.to_string();
        name.push_str(&name_str);

        let buf = &buf[name_size..];

        let end: &RawEntryEnd<OFFT> = reinterpret(buf);
        let compression = end.flags.get().into();

        let ret = Entry { name, offset: end.offset.as_u64(), length: end.length.as_u64(), compression };

        // if real_buf.is_empty() {
        reader.consume(entry_size);
        // }
        Ok(ret)
    }

    fn extract_lzma<W: Write, R: io::BufRead>(mut reader: R, mut writer: W, length: u64) -> Result<()> {
        #[repr(C)]
        struct LzmaHeader {
            unpacked_size:     U32<LE>,
            properties_length: U32<LE>,
        };

        let mut header_buf: [u8; size_of::<LzmaHeader>()] = [0; size_of::<LzmaHeader>()];
        reader.read_exact(&mut header_buf[..])?;
        let header: &LzmaHeader = reinterpret(&header_buf);
        assert_eq!(header.properties_length.get(), 5);

        let compressed_length = length - (size_of::<LzmaHeader>() as u64);

        // FIXME: wtf
        let uncompressed_length = header.unpacked_size.get();
        // let uncompressed_length = header.unpacked_size.get() - 1;

        let mut buf: Vec<u8> = Vec::with_capacity(compressed_length as usize);
        buf.resize_with(buf.capacity(), || 0);
        reader.read_exact(buf.as_mut_slice())?;
        let mut cur = io::Cursor::new(buf);

        let opts =
            lzma_rs::decompress::Options { unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed_length as _)) };
        lzma_rs::lzma_decompress_with_options(&mut cur, &mut writer, &opts).map_err(|err| anyhow!("{:?}", err))?;

        Ok(())
    }

    fn extract_direct<W: Write, R: io::BufRead>(mut reader: R, mut writer: W, length: u64) -> Result<()> {
        let mut buf: [u8; 100_000] = [0; 100000];
        let mut remain = length as usize;

        while remain > 0 {
            let buf_len = std::cmp::min(buf.len(), remain);
            let read = reader.read(&mut buf[..buf_len])?;

            let read_buf = &buf[..read];
            writer.write_all(&read_buf)?;
            remain -= read;
        }

        Ok(())
    }

    fn extract_to_inner<W: Write>(&mut self, writer: W, length: u64, offset: u64, compression: Compression) -> Result<()> {
        self.reader.seek(io::SeekFrom::Start(offset))?;

        match compression {
            Compression::None => Self::extract_direct(&mut self.reader, writer, length),
            Compression::LZMA => Self::extract_lzma(&mut self.reader, writer, length),
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
