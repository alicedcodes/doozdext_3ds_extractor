use std::{fs::File, path::Path};

use binrw::{BinRead, FilePtr};
use color_eyre::Result;
use indexmap::IndexMap;
use serde::Serialize;
use std::io::Write;

use crate::{garc, garc_files, text::TextFile, text_ids, to_id};

#[allow(dead_code)]
#[derive(BinRead, Debug)]
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
    Boolean(bool),
}

#[derive(Serialize)]
#[serde(untagged)]
enum MoveJsMultihit {
    Number(i32),
    Range(i32, i32),
}

#[allow(non_snake_case)]
#[serde_with::skip_serializing_none]
#[derive(Serialize)]
struct MoveJs {
    num: u32,
    acc: MoveJsAccuracy,
    power: u32,
    category: String,
    name: String,
    pp: u32,
    priority: i32,
    crit: i32,
    r#type: String,
    target: String,
    desc: String,

    multihit: Option<MoveJsMultihit>,
    zPower: Option<i32>,
}

fn get_multihit(move_stats: &MoveStats) -> Option<MoveJsMultihit> {
    use MoveJsMultihit::*;
    let min = move_stats.hit_min_max & 0xf;
    let max = move_stats.hit_min_max >> 4;
    match (min, max) {
        (0, 0) => None,
        (a, b) if a == b => Some(Number(a as i32)),
        (a, b) => Some(Range(a as i32, b as i32)),
    }
}

pub fn dump_moves(rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;
    let move_descs = &text_files[text_ids::MOVE_DESCS].lines;
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;

    let move_path = rom_path.join(garc_files::BASE_PATH).join(garc_files::MOVE);
    let moves =
        &garc::read_files::<BinLinkedMoves>(&garc::GarcFile::read_le(&mut File::open(move_path)?)?)
            [0]
        .files;
    let move_map: IndexMap<String, MoveJs> = moves
        .iter()
        .enumerate()
        .map(|(index, cmove)| {
            let name = &move_names[index];
            (
                to_id(name.clone()),
                MoveJs {
                    num: index as _,
                    name: name.clone(),
                    acc: match cmove.accuracy {
                        101 => MoveJsAccuracy::Boolean(true),
                        a => MoveJsAccuracy::Number(a as _),
                    },
                    power: cmove.power as _,
                    pp: cmove.pp as _,
                    category: match cmove.category {
                        1 => "Physical",
                        2 => "Special",
                        _ => "Status",
                    }
                    .to_owned(),
                    priority: cmove.priority as _,
                    crit: (cmove.crit_stage as i32) + 1,
                    r#type: type_names[cmove.move_type as usize].clone(),
                    target: match cmove.target {
                        1 | 2 => "adjacentAlly",
                        4 => "allAdjacent",
                        5 => "allAdjacentFoes",
                        7 => "self",
                        9 => "randomNormal",
                        _ => "normal",
                    }
                    .to_owned(),
                    multihit: get_multihit(cmove),
                    zPower: Some(cmove.z_power as i32),
                    desc: move_descs[index].clone(),
                },
            )
        })
        .skip(1)
        .collect();

    let mut f = File::create(out_path.join("moves.json"))?;
    write!(f, "{}", serde_json::to_string(&move_map)?)?;
    Ok(())
}
