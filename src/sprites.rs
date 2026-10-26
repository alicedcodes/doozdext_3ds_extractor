use crate::{
    PokemonStats,
    garc::{self, GarcFile},
    garc_files,
    pokemon::PokeEntry,
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
const NORMAL_FORM_COUNT: usize = 808;

const FORM_FIRST: &[usize] = &[
    19, 20, 26, 27, 28, 37, 38, 50, 51, 52, 53, 74, 75, 76, 88, 89, 103, 105,
];

const FORM_ORDER: &[(usize, &[usize])] = &[
    (386, &[2, 0, 1, 3]),
    (412, &[1]),
    (413, &[2, 1, 0]),
    (479, &[4, 3, 5, 0, 2, 1]),
    (585, &[3]),
    (586, &[3]),
    (648, &[1, 0]),
];

const PELAGO: &[usize] = &[
    57, 59, 65, 67, 80, 94, 96, 130, 143, 213, 228, 230, 244, 261, 264, 266, 268, 270, 272, 274,
    276, 279, 281, 283, 286, 289, 291, 293, 295, 299, 304, 322, 324, 435, 459, 461, 479, 490, 492,
    514, 535, 553, 555, 627, 629, 647, 663, 672, 686, 688, 705, 707, 718, 695, 729, 752, 761, 769,
    783, 784, 786, 794, 796, 798, 805, 821, 823, 850, 852, 854, 857, 859, 861,
];

const MANUAL_FROM: usize = 650;

const MANUAL: &[(&str, usize)] = &[
    ("chespin", 929),
    ("quilladin", 930),
    ("chesnaught", 931),
    ("fennekin", 926),
    ("braixen", 927),
    ("delphox", 928),
    ("froakie", 932),
    ("frogadier", 933),
    ("greninja", 934),
    ("greninjaash", 938),
    ("bunnelby", 907),
    ("diggersby", 908),
    ("fletchling", 969),
    ("fletchinder", 970),
    ("talonflame", 971),
    ("scatterbug", 883),
    ("spewpa", 884),
    ("vivillon", 891),
    ("litleo", 880),
    ("pyroar", 882),
    ("flabebe", 911),
    ("floette", 917),
    ("floetteeternal", 915),
    ("florges", 921),
    ("skiddo", 942),
    ("gogoat", 943),
    ("pancham", 968),
    ("pangoro", 944),
    ("furfrou", 872),
    ("furfrouheart", 870),
    ("furfroustar", 875),
    ("furfroudiamond", 868),
    ("furfroudebutante", 876),
    ("furfroumatron", 871),
    ("furfroudandy", 869),
    ("furfroulareine", 867),
    ("furfroukabuki", 873),
    ("furfroupharaoh", 874),
    ("espurr", 947),
    ("meowstic", 949),
    ("meowsticf", 948),
    ("honedge", 959),
    ("doublade", 960),
    ("aegislash", 961),
    ("aegislashblade", 962),
    ("spritzee", 976),
    ("aromatisse", 977),
    ("swirlix", 957),
    ("slurpuff", 958),
    ("inkay", 940),
    ("malamar", 941),
    ("binacle", 963),
    ("barbaracle", 964),
    ("skrelp", 905),
    ("dragalge", 906),
    ("clauncher", 972),
    ("clawitzer", 974),
    ("helioptile", 945),
    ("heliolisk", 946),
    ("tyrunt", 952),
    ("tyrantrum", 953),
    ("amaura", 954),
    ("aurorus", 955),
    ("sylveon", 986),
    ("hawlucha", 980),
    ("dedenne", 956),
    ("carbink", 985),
    ("goomy", 982),
    ("sliggoo", 983),
    ("goodra", 984),
    ("klefki", 978),
    ("phantump", 878),
    ("trevenant", 879),
    ("pumpkaboo", 950),
    ("gourgeistsmall", 951),
    ("gourgeistsuper", 951),
    ("bergmite", 966),
    ("avalugg", 967),
    ("noibat", 925),
    ("noivern", 981),
    ("zygarde10", 993),
    ("zygarde50", 995),
    ("diancie", 1001),
    ("dianciemega", 1002),
    ("hoopa", 1005),
    ("hoopaunbound", 1004),
    ("volcanion", 1003),
    //
    //
    ("rowlet", 1090),
    ("dartrix", 1091),
    ("decidueye", 1093),
    ("litten", 1094),
    ("torracat", 1095),
    ("incineroar", 1096),
    ("popplio", 1097),
    ("brionne", 1098),
    ("primarina", 1099),
    ("pikipek", 1013),
    ("trumbeak", 1014),
    ("toucannon", 1015),
    ("yungoos", 1069),
    ("gumshoos", 1070),
    ("gumshoostotem", 1071),
    ("grubbin", 1084),
    ("charjabug", 1085),
    ("vikavolt", 1086),
    ("vikavolttotem", 1087),
    ("crabrawler", 1112),
    ("crabominable", 1113),
    ("oricorio", 1066),
    ("oricoriopompom", 1065),
    ("oricoriopau", 1067),
    ("oricoriosensu", 1068),
    ("cutiefly", 1081),
    ("ribombee", 1082),
    ("ribombeetotem", 1083),
    ("rockruff", 1072),
    ("lycanroc", 1074),
    ("lycanrocmidnight", 1075),
    ("lycanrocdusk", 1076),
    ("wishiwashi", 1033),
    ("wishiwashischool", 1032),
    ("mareanie", 1006),
    ("toxapex", 1007),
    ("mudbray", 1088),
    ("mudsdale", 1089),
    ("dewpider", 1100),
    ("araquanid", 1101),
    ("araquanidtotem", 1102),
    ("fomantis", 1128),
    ("lurantis", 1129),
    ("lurantistotem", 1130),
    ("morelull", 1008),
    ("shiinotic", 1009),
    ("salandit", 1010),
    ("salazzle", 1011),
    ("salazzletotem", 1012),
    ("stufful", 1103),
    ("bewear", 1104),
    ("bounsweet", 1018),
    ("steenee", 1019),
    ("tsareena", 1021),
    ("comfey", 1116),
    ("oranguru", 1016),
    ("passimian", 1023),
    ("wimpod", 1126),
    ("golisopod", 1127),
    ("sandygast", 1035),
    ("palossand", 1036),
    ("pyukumuku", 1034),
    ("typenull", 1114),
    ("silvally", 1115),
    ("minior", 1043),
    ("miniorred", 1061),
    ("miniororange", 1057),
    ("minioryellow", 1063),
    ("miniorgreen", 1039),
    ("miniorblue", 1037),
    ("miniorindigo", 1041),
    ("miniorviolet", 1059),
    ("komala", 1017),
    ("turtonator", 1105),
    ("togedemaru", 1110),
    ("togedemarutotem", 1111),
    ("mimikyu", 1028),
    ("mimikyubusted", 1029),
    ("mimikyutotem", 1030),
    ("mimikyubustedtotem", 1031),
    ("bruxish", 1106),
    ("drampa", 1107),
    ("dhelmise", 1108),
    ("jangmoo", 1024),
    ("hakamoo", 1025),
    ("kommoo", 1026),
    ("kommoototem", 1027),
    ("tapukoko", 1077),
    ("tapulele", 1078),
    ("tapubulu", 1079),
    ("tapufini", 1080),
    ("cosmog", 1131),
    ("cosmoem", 1132),
    ("solgaleo", 1133),
    ("lunala", 1134),
    ("nihilego", 1135),
    ("buzzwole", 1137),
    ("pheromosa", 1139),
    ("xurkitree", 1138),
    ("celesteela", 1141),
    ("kartana", 1140),
    ("guzzlord", 1136),
    ("necrozma", 1124),
    ("magearna", 1143),
    ("marshadow", 1144),
    ("poipole", 1150),
    ("naganadel", 1151),
    ("stakataka", 1146),
    ("blacephalon", 1148),
    ("zeraora", 1152),
];

const EXTRA_SLOTS: &[(usize, isize)] = &[(25, 2), (247, -1), (414, -2), (493, -17), (649, 4)];

fn slots(species: usize, form_count: u8) -> usize {
    let extra = EXTRA_SLOTS
        .iter()
        .find(|(s, _)| *s == species)
        .map_or(0, |(_, e)| *e);
    ((form_count as isize + extra).max(1)) as usize
}

fn species_and_form(pokemons: &[PokemonStats], rom_index: usize) -> (usize, usize) {
    if rom_index < NORMAL_FORM_COUNT {
        return (rom_index, 0);
    }
    let (s, p) = pokemons
        .iter()
        .enumerate()
        .take(NORMAL_FORM_COUNT)
        .find(|(_, p)| {
            let start = p.form_stats_id as usize;
            p.form_count > 1 && rom_index >= start && rom_index < start + p.form_count as usize - 1
        })
        .unwrap();
    (s, rom_index - p.form_stats_id as usize + 1)
}

fn sprite_index(pokemons: &[PokemonStats], species: usize, mut form: usize) -> usize {
    if form >= slots(species, pokemons[species].form_count) {
        form = 0;
    }
    if FORM_FIRST.contains(&species) {
        form = match form {
            0 => 1,
            1 => 0,
            f => f,
        };
    }
    if let Some((_, order)) = FORM_ORDER.iter().find(|(s, _)| *s == species) {
        form = order[form];
    }
    let before: usize = pokemons[1..species]
        .iter()
        .enumerate()
        .map(|(i, p)| slots(i + 1, p.form_count))
        .sum();
    1 + before + form
}

pub fn dump_icons(rom_path: &Path, out_path: &Path, poke_names: &[PokeEntry]) -> Result<()> {
    let base = rom_path.join(garc_files::BASE_PATH);
    let icons = garc::read_files::<Icon>(&GarcFile::read_le(&mut File::open(
        base.join(garc_files::ICONS),
    )?)?);
    let pokemons = garc::read_files::<PokemonStats>(&GarcFile::read_le(&mut File::open(
        base.join(garc_files::POKEMON_STATS),
    )?)?);
    let keep: Vec<usize> = (0..icons.len()).filter(|i| !PELAGO.contains(i)).collect();

    let x = (64 - ICON_W) / 2;
    let y = (32 - ICON_H) / 2;

    let dir = out_path.join("icons");
    std::fs::create_dir_all(&dir)?;

    let mut used: Vec<usize> = Vec::new();
    for poke in poke_names {
        let (species, form) = species_and_form(&pokemons, poke.rom_index);
        let raw_index = if species >= MANUAL_FROM {
            match MANUAL.iter().find(|(id, _)| *id == poke.id) {
                Some(&(_, raw)) => raw,
                None => {
                    println!("unmapped: {}", poke.id);
                    continue;
                }
            }
        } else {
            let index = sprite_index(&pokemons, species, form);
            let Some(&raw) = keep.get(index) else {
                println!("{index} -> NONE - {}", poke.id);
                continue;
            };
            raw
        };
        used.push(raw_index);
        let Some(img) = icons[raw_index].0.as_ref() else {
            continue;
        };
        imageops::crop_imm(img, x, y, ICON_W, ICON_H)
            .to_image()
            .save(dir.join(format!("{}-{}.webp", poke.rom_index, poke.id)))?;
    }

    Ok(())
}
