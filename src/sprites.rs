use crate::{
    garc::{self, GarcFile},
    garc_files,
};
use binrw::{BinRead, BinResult, Endian};
use color_eyre::Result;
use image::{Rgba, RgbaImage, imageops};
use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};

fn lz11(data: &[u8]) -> Vec<u8> {
    let mut size = u32::from_le_bytes([data[1], data[2], data[3], 0]) as usize;
    let mut pos = 4;
    if size == 0 {
        size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        pos = 8;
    }
    let mut out: Vec<u8> = Vec::with_capacity(size);
    while out.len() < size {
        let flags = data[pos];
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(data[pos]);
                pos += 1;
                continue;
            }
            let b1 = data[pos] as usize;
            let (len, disp) = match b1 >> 4 {
                0 => {
                    let (b2, b3) = (data[pos + 1] as usize, data[pos + 2] as usize);
                    pos += 3;
                    (
                        (((b1 & 0xF) << 4) | (b2 >> 4)) + 0x11,
                        (((b2 & 0xF) << 8) | b3) + 1,
                    )
                }
                1 => {
                    let (b2, b3, b4) = (
                        data[pos + 1] as usize,
                        data[pos + 2] as usize,
                        data[pos + 3] as usize,
                    );
                    pos += 4;
                    (
                        (((b1 & 0xF) << 12) | (b2 << 4) | (b3 >> 4)) + 0x111,
                        (((b3 & 0xF) << 8) | b4) + 1,
                    )
                }
                n => {
                    let b2 = data[pos + 1] as usize;
                    pos += 2;
                    (n + 1, (((b1 & 0xF) << 8) | b2) + 1)
                }
            };
            for _ in 0..len {
                out.push(out[out.len() - disp]);
            }
        }
    }
    out.truncate(size);
    out
}

fn to_rgba(format: u8, px: &[u8]) -> [u8; 4] {
    let v = u16::from_le_bytes([px[0], px[1]]);
    match format {
        5 => {
            let (r, g, b) = (
                ((v >> 11) & 0x1F) as u8,
                ((v >> 5) & 0x3F) as u8,
                (v & 0x1F) as u8,
            );
            [
                (r << 3) | (r >> 2),
                (g << 2) | (g >> 4),
                (b << 3) | (b >> 2),
                255,
            ]
        }
        7 => {
            let ch = |shift: u32| {
                let c = ((v >> shift) & 0x1F) as u8;
                (c << 3) | (c >> 2)
            };
            [ch(11), ch(6), ch(1), if v & 1 != 0 { 255 } else { 0 }]
        }
        8 => {
            let ch = |shift: u32| ((v >> shift) & 0xF) as u8 * 17;
            [ch(12), ch(8), ch(4), ch(0)]
        }
        9 => [px[3], px[2], px[1], px[0]],
        _ => unimplemented!("unsupported bclim format {format}"),
    }
}

fn decode_bclim(data: &[u8]) -> RgbaImage {
    let footer = &data[data.len() - 0x28..];
    let format_offset = if &footer[..4] == b"FLIM" { 14 } else { 12 };
    let imag = &footer[0x14..];

    let width = u16::from_le_bytes([imag[8], imag[9]]) as usize;
    let height = u16::from_le_bytes([imag[10], imag[11]]) as usize;
    let format = imag[format_offset];
    let rotated = imag[format_offset + 1] & 4 != 0;

    let (w, h) = if rotated {
        (height, width)
    } else {
        (width, height)
    };
    let bytes_per_pixel = if format == 9 { 4 } else { 2 };

    let mut img = RgbaImage::new(w as u32, h as u32);
    let mut offset = 0;
    for tile_y in 0..h.next_power_of_two().max(8) / 8 {
        for tile_x in 0..w.next_power_of_two().max(8) / 8 {
            for i in 0..64usize {
                let x = tile_x * 8 + ((i & 1) | ((i >> 1) & 2) | ((i >> 2) & 4));
                let y = tile_y * 8 + (((i >> 1) & 1) | ((i >> 2) & 2) | ((i >> 3) & 4));
                let px = &data[offset..offset + bytes_per_pixel];
                offset += bytes_per_pixel;
                if x < w && y < h {
                    img.put_pixel(x as u32, y as u32, Rgba(to_rgba(format, px)));
                }
            }
        }
    }

    if rotated {
        imageops::rotate270(&img)
    } else {
        img
    }
}

struct Icon(Option<RgbaImage>);

impl BinRead for Icon {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        if data.is_empty() {
            return Ok(Icon(None));
        }
        if data[0] == 0x11 {
            data = lz11(&data);
        }
        Ok(Icon(Some(decode_bclim(&data))))
    }
}

const ICON_W: u32 = 40;
const ICON_H: u32 = 30;
const SHEET_COLUMNS: u32 = 32;

pub fn dump_icons(rom_path: &Path, out_path: &Path) -> Result<()> {
    let path = rom_path.join(garc_files::BASE_PATH).join(garc_files::ICONS);
    let icons = garc::read_files::<Icon>(&GarcFile::read_le(&mut File::open(path)?)?);

    let x = (64 - ICON_W) / 2;
    let y = (32 - ICON_H) / 2;

    let count = icons.len() as u32 - 1;
    let rows = count.div_ceil(SHEET_COLUMNS);
    let mut sheet = RgbaImage::new(SHEET_COLUMNS * ICON_W, rows * ICON_H);

    for (index, icon) in icons.iter().enumerate().skip(1) {
        if let Some(img) = &icon.0 {
            let cell = index as u32 - 1;
            let sprite = imageops::crop_imm(img, x, y, ICON_W, ICON_H).to_image();
            imageops::replace(
                &mut sheet,
                &sprite,
                ((cell % SHEET_COLUMNS) * ICON_W) as i64,
                ((cell / SHEET_COLUMNS) * ICON_H) as i64,
            );
        }
    }

    sheet.save(out_path.join("icons.webp"))?;
    Ok(())
}
