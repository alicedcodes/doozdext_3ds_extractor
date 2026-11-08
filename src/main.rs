use crate::{garc::GarcFile, pokemon::PokeEntry};
use binrw::BinRead;
use color_eyre::Result;
use serde_json::json;
use std::{
    env,
    fs::{self, File},
    path::Path,
};

mod garc;
mod learnset;
mod moves;
mod pokemon;
mod sprites;
mod text;

mod text_ids {
    pub const SPECIES_NAMES: usize = 60;

    pub const ABILITY_NAMES: usize = 101;

    pub const MOVE_NAMES: usize = 118;
    pub const MOVE_DESCS: usize = 117;

    pub const TYPE_NAMES: usize = 112;
}

mod garc_files {
    pub const BASE_PATH: &str = "romfs/a/";

    pub const MOVE: &str = "0/1/1";
    pub const LVL_UP_MOVES: &str = "0/1/3";

    pub const EVOLUTIONS: &str = "0/1/4";

    pub const POKEMON_STATS: &str = "0/1/7";

    pub const EN_TEXT: &str = "0/3/2";

    pub const ICONS: &str = "0/6/2";
}

#[allow(dead_code)]
#[derive(BinRead)]
struct Stats {
    hp: u8,
    atk: u8,
    def: u8,
    spe: u8,
    spa: u8,
    spd: u8,
}

#[allow(dead_code)]
#[derive(BinRead)]
struct PokemonStats {
    stats: Stats,
    types: (u8, u8),
    catch_rate: u8,
    evo_stage: u8,
    ev_yield: u16,
    items: [u16; 3],
    gender: u8,
    hatch_cycles: u8,
    base_friendship: u8,
    exp_growth: u8,
    egg_groups: [u8; 2],
    abilities: [u8; 3],
    escape_rate: u8,
    form_stats_id: u16,
    form_sprite: u16,
    form_count: u8,
    sprite_bits: u8,
    base_exp: u16,
    height: u16,
    weight: u16,
    tm_bits: [u8; 0x10],
    tutor_bits: [u8; 0x4],
    beach_bits: [u8; 0xa],
}

fn to_id(s: String) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit())
        .collect()
}

fn dump_abilities(text_files: &[text::TextFile]) -> Result<Vec<&str>> {
    let ability_names = &text_files[text_ids::ABILITY_NAMES].lines;

    Ok(ability_names
        .iter()
        .skip(1)
        .map(|name| name.as_str())
        .collect())
}

fn dump_types(text_files: &[text::TextFile]) -> Result<Vec<&str>> {
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;

    Ok(type_names.iter().map(|name| name.as_str()).collect())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args: Vec<String> = env::args().collect();
    let rom_path = Path::new(&args[1]);

    let out_path: &Path = match args.get(2) {
        Some(path_str) => Path::new(path_str),
        None => rom_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    };

    let name = rom_path
        .file_name()
        .map_or_else(|| "data".into(), |n| n.to_string_lossy());
    let data_file = out_path.join(format!("{name}.json"));
    let sprites_dir = out_path.join(format!("{name}_sprites"));

    let _ = fs::remove_file(&data_file);
    let _ = fs::remove_dir_all(&sprites_dir);
    fs::create_dir_all(out_path)?;

    let mut en_text_file = File::open(
        rom_path
            .join(garc_files::BASE_PATH)
            .join(garc_files::EN_TEXT),
    )?;
    let text_garc_file = GarcFile::read_le(&mut en_text_file)?;
    let text_files = garc::read_files::<text::TextFile>(&text_garc_file);

    let dex_map = pokemon::dump_pokes(rom_path, &text_files)?;
    let poke_names: Vec<PokeEntry> = dex_map
        .iter()
        .map(|(id, p)| PokeEntry {
            rom_index: p.index,
            id: id.clone(),
        })
        .collect();

    let learnsets = learnset::dump_learnsets(rom_path, &text_files, &poke_names)?;
    let moves = moves::dump_moves(rom_path, &text_files)?;
    let abilities = dump_abilities(&text_files)?;
    let types = dump_types(&text_files)?;

    let data = json!({
        "pokedex": dex_map.values().collect::<Vec<_>>(),
        "learnsets": learnsets,
        "moves": moves,
        "abilities": abilities,
        "types": types,
    });
    fs::write(&data_file, serde_json::to_string(&data)?)?;

    sprites::dump_icons(rom_path, &sprites_dir, &poke_names)?;
    Ok(())
}
