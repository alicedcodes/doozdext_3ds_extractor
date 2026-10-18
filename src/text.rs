use binrw::BinRead;

#[derive(BinRead)]
struct TextFileHeader {
    _text_sections: u16,
    line_count: u16,
    _total_length: u32,
    _initial_key: u32,
    section_data_offset: u32,
    _section_length: u32,
}

#[derive(BinRead)]
struct LineInfo {
    offset: u32,
    length: u32,
}

#[derive(BinRead)]
#[br(import(len:u32))]
struct EncryptedLine {
    #[br(count = len)]
    data: Vec<u16>,
}

impl EncryptedLine {
    fn into_string(self, mut key: u16) -> String {
        self.data
            .iter()
            .map_while(|u| {
                let c = char::from_u32((*u ^ key) as u32).unwrap_or(' ');
                key = key << 3 | key >> 13;
                match c {
                    '\0' => None,
                    '\u{E08E}' => Some('M'),
                    '\u{E08F}' => Some('F'),
                    'é' => Some('e'),
                    c => Some(c),
                }
            })
            .collect()
    }
}

pub struct TextFile {
    _header: TextFileHeader,
    pub lines: Vec<String>,
}

impl BinRead for TextFile {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let header = TextFileHeader::read_options(reader, endian, ())?;
        let mut lines: Vec<String> = Vec::new();
        let mut key: u16 = 0x7c89;
        reader.seek(std::io::SeekFrom::Start(
            header.section_data_offset as u64 + 4,
        ))?;
        for _ in 0..header.line_count {
            let line_info = LineInfo::read_options(reader, endian, ())?;
            let pos = reader.stream_position()?;
            reader.seek(std::io::SeekFrom::Start(
                line_info.offset as u64 + header.section_data_offset as u64,
            ))?;
            lines.push(
                EncryptedLine::read_options(reader, endian, (line_info.length,))?.into_string(key),
            );
            reader.seek(std::io::SeekFrom::Start(pos))?;
            key = key.wrapping_add(0x2983);
        }

        Ok(TextFile {
            _header: header,
            lines,
        })
    }
}
