use std::{fs::File, path::Path};

use binrw::{BinRead, FilePtr};
use color_eyre::Result;
use serde::Serialize;
use std::io::Write;

use crate::{garc, garc_files, text::TextFile, text_ids};

#[allow(dead_code)]
#[derive(BinRead)]
struct MoveStats {
    move_type: u8,
    quality: u8,
    category: u8,
    power: u8,

    accuracy: u8,
    pp: u8,
    priority: i8,
    hit_min_max: u8,

    inflict: u16,
    inflict_percent: u8,
    inflict_duration: u8,

    turn_min: u8,
    turn_max: u8,
    crit_stage: u8,
    flinch: u8,

    effect_sequence: u16,
    recoil_absorption: i8,
    heal: u8,

    target: u8,
    stat: [u8; 3],
    stat_stage: [i8; 3],
    stat_percent: [u8; 3],

    z_move: u16,
    z_power: u8,
    z_effect: u8,

    refresh_type: u8,
    refresh_percent: u8,

    flags: u32,
}

#[allow(dead_code)]
#[derive(BinRead)]
#[br(magic = b"WD")]
struct BinLinkedMoves {
    ccount: u16,
    #[br(count = ccount)]
    files: Vec<FilePtr<u32, MoveStats>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum MoveJsAccuracy {
    Number(i32),
}

#[allow(non_snake_case)]
#[serde_with::skip_serializing_none]
#[derive(Serialize)]
struct MoveJs {
    name: String,
    num: usize,
    r#type: u8,
    category: u8,
    power: u32,
    zPower: Option<i32>,
    acc: MoveJsAccuracy,
    pp: u32,
    desc: String,
}

pub fn dump_moves(rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;
    let move_descs = &text_files[text_ids::MOVE_DESCS].lines;

    let move_path = rom_path.join(garc_files::BASE_PATH).join(garc_files::MOVE);
    let moves =
        &garc::read_files::<BinLinkedMoves>(&garc::GarcFile::read_le(&mut File::open(move_path)?)?)
            [0]
        .files;

    let move_list: Vec<MoveJs> = moves
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, cmove)| MoveJs {
            name: move_names[index].clone(),
            num: index,
            r#type: cmove.move_type,
            category: cmove.category,
            power: cmove.power as _,
            zPower: Some(cmove.z_power as i32),
            acc: MoveJsAccuracy::Number(cmove.accuracy as _),
            pp: cmove.pp as _,
            desc: move_descs[index].replace('\n', " "),
        })
        .collect();

    let mut f = File::create(out_path.join("moves.json"))?;
    write!(f, "{}", serde_json::to_string(&move_list)?)?;
    Ok(())
}
