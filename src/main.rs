use crate::garc::GarcFile;
use binrw::BinRead;
use color_eyre::Result;
use std::{env, fs::File, path::Path};

mod garc;
mod learnset;
mod moves;
mod pokemon;
mod text;

mod text_ids {
    pub const SPECIES_NAMES: usize = 60;

    pub const ABILITY_NAMES: usize = 101;
    pub const ABILITY_DESCS: usize = 102;

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

fn dump_abilities(_rom_path: &Path, out_path: &Path, text_files: &[text::TextFile]) -> Result<()> {
    let ability_names = &text_files[text_ids::ABILITY_NAMES].lines;
    let ability_descs = &text_files[text_ids::ABILITY_DESCS].lines;

    let abilities: Vec<(&str, String)> = ability_names
        .iter()
        .zip(ability_descs)
        .skip(1)
        .map(|(name, desc)| (name.as_str(), desc.replace('\n', " ")))
        .collect();

    std::fs::write(
        out_path.join("abilities.json"),
        serde_json::to_string(&abilities)?,
    )?;
    Ok(())
}

const TYPE_COUNT: usize = 18;

fn read_type_chart(rom_path: &Path) -> Result<Vec<u8>> {
    let code = std::fs::read(rom_path.join("exefs/code.bin"))?;

    let mut sig = [4u8; TYPE_COUNT];
    sig[5] = 2;
    sig[7] = 0;
    sig[8] = 2;

    let start = code.windows(TYPE_COUNT).position(|w| w == sig).unwrap();

    Ok(code[start..start + TYPE_COUNT * TYPE_COUNT].to_vec())
}

fn dump_types(rom_path: &Path, out_path: &Path, text_files: &[text::TextFile]) -> Result<()> {
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;
    let type_chart = read_type_chart(rom_path)?;

    let types: Vec<(&str, Vec<u8>)> = type_names
        .iter()
        .take(TYPE_COUNT)
        .enumerate()
        .map(|(defender, name)| {
            let damage_taken = (0..TYPE_COUNT)
                .map(|attacker| type_chart[attacker * TYPE_COUNT + defender] / 2)
                .collect();
            (name.as_str(), damage_taken)
        })
        .collect();

    std::fs::write(out_path.join("types.json"), serde_json::to_string(&types)?)?;
    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args: Vec<String> = env::args().collect();
    let rom_path = Path::new(&args[1]);
    let out_path = Path::new(&args[2]);
    std::fs::create_dir_all(out_path)?;

    let mut en_text_file = File::open(rom_path.join(garc_files::BASE_PATH).join("0/3/2"))?;
    let text_garc_file = GarcFile::read_le(&mut en_text_file)?;
    let text_files = garc::read_files::<text::TextFile>(&text_garc_file);

    let poke_names = pokemon::dump_pokes(&rom_path, &out_path, &text_files)?;
    learnset::dump_learnsets(&rom_path, &out_path, &text_files, &poke_names)?;
    moves::dump_moves(rom_path, out_path, &text_files)?;
    dump_abilities(rom_path, out_path, &text_files)?;
    dump_types(rom_path, out_path, &text_files)?;

    Ok(())
}
