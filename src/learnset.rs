use crate::{
    PokemonStats,
    garc::{self, GarcFile},
    garc_files,
    pokemon::PokeEntry,
    text::TextFile,
    text_ids, to_id,
};
use binrw::{BinRead, helpers::until_eof};
use color_eyre::Result;
use serde::Serialize;
use std::{collections::HashMap, fs::File, path::Path};

#[derive(BinRead)]
struct LevelUpMove {
    move_id: i16,
    level: i16,
}

#[derive(BinRead)]
struct LevelUpMoves {
    #[br(parse_with = until_eof)]
    lvl_moves: Vec<LevelUpMove>,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Clone, Copy)]
enum Method {
    tm,
    tutor,
}

#[derive(Serialize)]
#[serde(untagged)]
enum LearnsetVariant {
    How(Method),
    Level(i16),
}

#[derive(Serialize)]
pub struct LearnsetEntry(u16, LearnsetVariant);

#[derive(Serialize)]
struct Learnset(Vec<LearnsetEntry>);

impl Learnset {
    fn merge(mut self, mut other: Learnset) -> Self {
        self.0.append(&mut other.0);
        self
    }
}

pub fn dump_learnsets(
    rom_path: &Path,
    out_path: &Path,
    text_files: &[TextFile],
    poke_names: &[PokeEntry],
) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;

    let mut name_to_index: HashMap<String, u16> = HashMap::new();
    for (index, name) in move_names.iter().enumerate().skip(1) {
        name_to_index
            .entry(to_id(name.clone()))
            .or_insert(index as u16 - 1);
    }
    let tm_moves: Vec<Option<u16>> = TMS
        .iter()
        .map(|name| name_to_index.get(&to_id(name.to_string())).copied())
        .collect();

    let lvl_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::LVL_UP_MOVES);
    let lvl_ups = garc::read_files::<LevelUpMoves>(&GarcFile::read_le(&mut File::open(lvl_path)?)?);

    let pokemon_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::POKEMON_STATS);
    let pokemons =
        garc::read_files::<PokemonStats>(&GarcFile::read_le(&mut File::open(pokemon_path)?)?);

    let learnsets: Vec<Option<Learnset>> = poke_names
        .iter()
        .map(|poke| {
            if COMBAT_FORMES.contains(&poke.id.as_str()) {
                return None;
            }
            let stats = &pokemons[poke.rom_index];
            Some(
                make_lvl_up_learnset(&lvl_ups[poke.rom_index])
                    .merge(make_bit_learnset(
                        &stats.tm_bits,
                        tm_moves.iter().copied(),
                        Method::tm,
                    ))
                    .merge(make_bit_learnset(
                        &stats.beach_bits,
                        BEACH_TUTORS.iter().map(|&id| Some(id as u16 - 1)),
                        Method::tutor,
                    ))
                    .merge(make_bit_learnset(
                        &stats.tutor_bits,
                        MOVE_TUTORS.iter().map(|&id| Some(id as u16 - 1)),
                        Method::tutor,
                    )),
            )
        })
        .collect();

    std::fs::write(
        out_path.join("learnsets.json"),
        serde_json::to_string(&learnsets)?,
    )?;
    Ok(())
}

fn make_lvl_up_learnset(lvl_ups: &LevelUpMoves) -> Learnset {
    Learnset(
        lvl_ups
            .lvl_moves
            .iter()
            .take_while(|lvl_up| lvl_up.move_id > 0)
            .map(|lvl_up| {
                LearnsetEntry(
                    lvl_up.move_id as u16 - 1,
                    LearnsetVariant::Level(lvl_up.level),
                )
            })
            .collect(),
    )
}

fn check_bit(bits: &[u8], index: usize) -> bool {
    bits[index / 8] & (1 << (index % 8)) != 0
}

fn make_bit_learnset(
    bits: &[u8],
    moves: impl Iterator<Item = Option<u16>>,
    how: Method,
) -> Learnset {
    Learnset(
        moves
            .enumerate()
            .filter(|(index, _)| check_bit(bits, *index))
            .filter_map(|(_, move_)| move_)
            .map(|move_| LearnsetEntry(move_, LearnsetVariant::How(how)))
            .collect(),
    )
}

const TMS: &[&str] = &[
    "Work Up",
    "Dragon Dance",
    "Take Down",
    "Psychic Fangs",
    "Weather Ball",
    "Earthquake",
    "Ice Fang",
    "Power-Up Punch",
    "Venoshock",
    "Hidden Power",
    "Fire Fang",
    "Nasty Plot",
    "Ice Beam",
    "Blizzard",
    "Rest",
    "Light Screen",
    "Sleep Talk",
    "Rain Dance",
    "Electric Terrain",
    "Sunny Day",
    "Solar Beam",
    "Energy Ball",
    "Rock Tomb",
    "Megaton Kick",
    "Thunder",
    "Thunderbolt",
    "Solar Blade",
    "Rock Slide",
    "Retaliate",
    "Swords Dance",
    "Grassy Terrain",
    "Scorching Sands",
    "Reflect",
    "Sludge Bomb",
    "Close Combat",
    "Sludge Wave",
    "Charge Beam",
    "Fire Blast",
    "Burning Malice",
    "Substitute",
    "Taunt",
    "Will-O-Wisp",
    "Synchronoise",
    "Agility",
    "Sucker Punch",
    "Grass Knot",
    "Mystical Fire",
    "Ominous Wind",
    "Endure",
    "Flamethrower",
    "Smart Strike",
    "Aura Sphere",
    "Power Whip",
    "Brick Break",
    "Hydro Pump",
    "Hone Claws",
    "Belch",
    "Steel Wing",
    "Dark Pulse",
    "Parting Shot",
    "Megahorn",
    "Play Rough",
    "Flash Cannon",
    "Bulk Up",
    "Shadow Punch",
    "Blaze Kick",
    "Seismic Fist",
    "Giga Impact",
    "Sandstorm",
    "Hail",
    "Volt Switch",
    "Acrobatics",
    "Natural Gift",
    "Rock Polish",
    "Toxic Spikes",
    "Surf",
    "Poison Fang",
    "Thunder Fang",
    "Aurora Veil",
    "Rock Climb",
    "Wild Charge",
    "Lunge",
    "Bulldoze",
    "Poison Jab",
    "Calm Mind",
    "Nature Power",
    "Hex",
    "Rage",
    "U-turn",
    "Hyper Beam",
    "Strength",
    "Psychic",
    "Stone Edge",
    "Roost",
    "First Impression",
    "Dazzling Gleam",
    "Shadow Ball",
    "Hurricane",
    "Focus Blast",
    "Protect",
];

#[allow(clippy::zero_prefixed_literal)]
const BEACH_TUTORS: &[u16] = &[
    450, 343, 162, 530, 324, 442, 402, 529, 340, 067, 441, 253, 009, 007, 008, 277, 335, 414, 492,
    356, 393, 334, 387, 276, 527, 196, 401, 428, 406, 304, 231, 020, 173, 282, 235, 257, 272, 215,
    366, 143, 220, 202, 409, 264, 351, 352, 380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278,
    289, 446, 285, 477, 502, 432, 710, 707, 675, 673,
];

const MOVE_TUTORS: &[u16] = &[520, 519, 518, 338, 307, 308, 434, 620];

const COMBAT_FORMES: &[&str] = &[
    "minior",
    "venusaurmega",
    "charizardmegax",
    "charizardmegay",
    "blastoisemega",
    "beedrillmega",
    "pidgeotmega",
    "alakazammega",
    "slowbromega",
    "gengarmega",
    "kangaskhanmega",
    "pinsirmega",
    "gyaradosmega",
    "aerodactylmega",
    "ampharosmega",
    "steelixmega",
    "scizormega",
    "heracrossmega",
    "houndoommega",
    "tyranitarmega",
    "sceptilemega",
    "blazikenmega",
    "swampertmega",
    "gardevoirmega",
    "sableyemega",
    "mawilemega",
    "aggronmega",
    "medichammega",
    "manectricmega",
    "sharpedomega",
    "cameruptmega",
    "altariamega",
    "banettemega",
    "absolmega",
    "glaliemega",
    "salamencemega",
    "metagrossmega",
    "latiasmega",
    "latiosmega",
    "lopunnymega",
    "garchompmega",
    "lucariomega",
    "abomasnowmega",
    "gallademega",
    "audinomega",
    "dianciemega",
    "mimikyubusted",
    "mimikyubustedtotem",
    "wishiwashischool",
    "shayminsky",
    "darmanitanzen",
    "greninjaash",
    "kyuremwhite",
    "kyuremblack",
    "aegislashblade",
    "meloettapirouette",
    "cherrimsunshine",
    "castformsunny",
    "castformsnowy",
    "castformrainy",
];
