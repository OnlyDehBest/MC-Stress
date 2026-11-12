use std::collections::HashMap;

use crate::protocol::PayloadReader;
use crate::world_blocks::{self, Range};

// Overworld assumption (vanilla 1.18+). Custom-height dimensions or the Nether
// would need the value from the login/registry; -64 covers the common SMP case.
const MIN_Y: i32 = -64;
const AIR: u32 = 0;

enum Section {
    Uniform(u32),
    Paletted {
        bits: u8,
        palette: Option<Vec<u32>>,
        data: Vec<u64>,
    },
}

impl Section {
    fn block(&self, x: usize, y: usize, z: usize) -> u32 {
        match self {
            Section::Uniform(value) => *value,
            Section::Paletted { bits, palette, data } => {
                let index = (y * 16 + z) * 16 + x;
                let per_long = 64 / (*bits as usize);
                if per_long == 0 {
                    return AIR;
                }
                let long_index = index / per_long;
                let Some(word) = data.get(long_index) else {
                    return AIR;
                };
                let offset = (index % per_long) * (*bits as usize);
                let mask = (1u64 << bits) - 1;
                let raw = ((word >> offset) & mask) as u32;
                match palette {
                    Some(table) => *table.get(raw as usize).unwrap_or(&AIR),
                    None => raw,
                }
            }
        }
    }
}

struct Column {
    sections: Vec<Option<Section>>,
}

pub struct World {
    chunks: HashMap<(i32, i32), Column>,
    solid: &'static [Range],
}

impl World {
    pub fn new(protocol: i32) -> World {
        World {
            chunks: HashMap::new(),
            solid: world_blocks::solid_ranges(protocol),
        }
    }

    #[allow(dead_code)]
    pub fn loaded_chunks(&self) -> usize {
        self.chunks.len()
    }

    fn is_solid_state(&self, state: u32) -> bool {
        if state == AIR {
            return false;
        }
        self.solid
            .binary_search_by(|(lo, hi)| {
                if state < *lo {
                    std::cmp::Ordering::Greater
                } else if state > *hi {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    fn state_at(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        let column = self.chunks.get(&(x >> 4, z >> 4))?;
        let section_index = ((y - MIN_Y) >> 4) as usize;
        let section = column.sections.get(section_index)?.as_ref()?;
        let lx = (x & 15) as usize;
        let ly = (y & 15) as usize;
        let lz = (z & 15) as usize;
        Some(section.block(lx, ly, lz))
    }

    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.state_at(x, y, z).map(|s| self.is_solid_state(s)).unwrap_or(false)
    }

    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&(x >> 4, z >> 4))
    }

    /// Y of the first standable surface at or below `near_y` for column (x, z):
    /// the air block whose block below is solid. None if the column is unknown
    /// or has no solid block under `near_y`.
    pub fn surface_y(&self, x: i32, near_y: i32, z: i32) -> Option<i32> {
        if !self.is_loaded(x, z) {
            return None;
        }
        let start = near_y.clamp(MIN_Y + 1, MIN_Y + 16 * 24);
        let mut y = start;
        while y > MIN_Y {
            if self.is_solid(x, y - 1, z) && !self.is_solid(x, y, z) {
                return Some(y);
            }
            y -= 1;
        }
        None
    }

    /// Is the 2-block-tall space at (x, y, z) free to stand in?
    pub fn passable_column(&self, x: i32, y: i32, z: i32) -> bool {
        !self.is_solid(x, y, z) && !self.is_solid(x, y + 1, z)
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) {
        let Some(column) = self.chunks.get_mut(&(x >> 4, z >> 4)) else {
            return;
        };
        let section_index = ((y - MIN_Y) >> 4) as usize;
        if section_index >= column.sections.len() {
            return;
        }
        let slot = &mut column.sections[section_index];
        if let Some(Section::Paletted { bits, palette, data }) = slot.as_mut() {
            if write_block(*bits, palette, data, x, y, z, state) {
                return;
            }
        }
        // Uniform/empty section, or a palette that can't hold the new state:
        // copy every existing block into a dense 15-bit container, then write
        // the single changed block so the rest of the section is preserved.
        let previous = slot.take();
        let mut data = vec![0u64; DENSE_LONGS];
        for index in 0..4096usize {
            let lx = index & 15;
            let lz = (index >> 4) & 15;
            let ly = index >> 8;
            let current = match &previous {
                Some(section) => section.block(lx, ly, lz),
                None => AIR,
            };
            put_dense(&mut data, index, current);
        }
        let palette: Option<Vec<u32>> = None;
        write_block(15, &palette, &mut data, x, y, z, state);
        *slot = Some(Section::Paletted { bits: 15, palette, data });
    }

    pub fn unload(&mut self, x: i32, z: i32) {
        self.chunks.remove(&(x, z));
    }

    /// Parse a clientbound Chunk Data payload (protocol >= 770 format).
    pub fn load_chunk(&mut self, body: &[u8]) {
        if let Err(()) = self.try_load_chunk(body) {
            // Malformed or unexpected layout: drop it rather than corrupt the map.
        }
    }

    fn try_load_chunk(&mut self, body: &[u8]) -> Result<(), ()> {
        let mut reader = PayloadReader::new(body);
        let cx = reader.read_i32().map_err(|_| ())?;
        let cz = reader.read_i32().map_err(|_| ())?;

        // heightmaps: array of { type: varint, data: varint-prefixed i64[] }
        let heightmaps = reader.read_varint().map_err(|_| ())?;
        for _ in 0..heightmaps {
            reader.read_varint().map_err(|_| ())?;
            let longs = reader.read_varint().map_err(|_| ())? as usize;
            reader.skip(longs * 8).map_err(|_| ())?;
        }

        let data_len = reader.read_varint().map_err(|_| ())? as usize;
        let chunk_bytes = reader.read_bytes(data_len).map_err(|_| ())?;

        let mut section_reader = PayloadReader::new(chunk_bytes);
        let mut sections = Vec::new();
        while section_reader.remaining() > 0 {
            let _block_count = section_reader.read_i16().map_err(|_| ())?;
            let section = parse_block_states(&mut section_reader)?;
            skip_paletted(&mut section_reader, 3)?; // biomes
            sections.push(Some(section));
        }

        self.chunks.insert((cx, cz), Column { sections });
        Ok(())
    }
}

fn parse_block_states(reader: &mut PayloadReader) -> Result<Section, ()> {
    let bits = reader.read_u8().map_err(|_| ())?;
    if bits == 0 {
        let value = reader.read_varint().map_err(|_| ())? as u32;
        let longs = reader.read_varint().map_err(|_| ())? as usize;
        reader.skip(longs * 8).map_err(|_| ())?;
        return Ok(Section::Uniform(value));
    }
    let palette = if bits <= 8 {
        let len = reader.read_varint().map_err(|_| ())? as usize;
        let mut table = Vec::with_capacity(len);
        for _ in 0..len {
            table.push(reader.read_varint().map_err(|_| ())? as u32);
        }
        Some(table)
    } else {
        None
    };
    let longs = reader.read_varint().map_err(|_| ())? as usize;
    let mut data = Vec::with_capacity(longs);
    for _ in 0..longs {
        data.push(reader.read_i64().map_err(|_| ())? as u64);
    }
    Ok(Section::Paletted { bits, palette, data })
}

fn skip_paletted(reader: &mut PayloadReader, max_indirect_bits: u8) -> Result<(), ()> {
    let bits = reader.read_u8().map_err(|_| ())?;
    if bits == 0 {
        reader.read_varint().map_err(|_| ())?;
    } else if bits <= max_indirect_bits {
        let len = reader.read_varint().map_err(|_| ())? as usize;
        for _ in 0..len {
            reader.read_varint().map_err(|_| ())?;
        }
    }
    let longs = reader.read_varint().map_err(|_| ())? as usize;
    reader.skip(longs * 8).map_err(|_| ())?;
    Ok(())
}

const DENSE_LONGS: usize = (4096 + (64 / 15) - 1) / (64 / 15);

fn put_dense(longs: &mut [u64], index: usize, value: u32) {
    let per_long = 64 / 15;
    let long_index = index / per_long;
    let offset = (index % per_long) * 15;
    let mask = (1u64 << 15) - 1;
    longs[long_index] &= !(mask << offset);
    longs[long_index] |= ((value as u64) & mask) << offset;
}

/// Writes `state` into an existing paletted section. Returns false when the
/// section's palette cannot represent the state, so the caller can rebuild it.
fn write_block(bits: u8, palette: &Option<Vec<u32>>, data: &mut [u64], x: i32, y: i32, z: i32, state: u32) -> bool {
    let index = ((y & 15) as usize) * 256 + ((z & 15) as usize) * 16 + (x & 15) as usize;
    let bits = bits as usize;
    let per_long = 64 / bits;
    if per_long == 0 {
        return false;
    }
    let value = match palette {
        Some(table) => match table.iter().position(|v| *v == state) {
            Some(position) => position as u64,
            None => return false,
        },
        None => state as u64,
    };
    let long_index = index / per_long;
    if long_index >= data.len() {
        return false;
    }
    let offset = (index % per_long) * bits;
    let mask = (1u64 << bits) - 1;
    data[long_index] &= !(mask << offset);
    data[long_index] |= (value & mask) << offset;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense_section(fill: u32) -> Section {
        let mut data = vec![0u64; DENSE_LONGS];
        for index in 0..4096usize {
            put_dense(&mut data, index, fill);
        }
        Section::Paletted { bits: 15, palette: None, data }
    }

    #[test]
    fn indirect_palette_decode() {
        // bits = 4, per_long = 16, palette maps index 1 -> state 100.
        let mut word = 0u64;
        word |= 1 << (0 * 4); // (0,0,0) -> palette index 1
        word |= 1 << (5 * 4); // index 5 = (0,0,5)? -> arbitrary, just another cell
        let section = Section::Paletted {
            bits: 4,
            palette: Some(vec![0, 100]),
            data: vec![word],
        };
        assert_eq!(section.block(0, 0, 0), 100);
        assert_eq!(section.block(1, 0, 0), 0);
    }

    #[test]
    fn uniform_decode() {
        let section = Section::Uniform(42);
        assert_eq!(section.block(3, 7, 9), 42);
    }

    #[test]
    fn surface_and_solid() {
        let mut world = World::new(774);
        // One solid uniform section (state 1 = stone range) at the bottom.
        let mut sections: Vec<Option<Section>> = (0..24).map(|_| None).collect();
        sections[0] = Some(Section::Uniform(1));
        world.chunks.insert((0, 0), Column { sections });

        assert!(world.is_solid(5, -64, 5)); // bottom section is solid
        assert!(!world.is_solid(5, 40, 5)); // empty section above -> air
        // Surface sits just above the top of the solid section (y = -64..-49).
        assert_eq!(world.surface_y(5, 100, 5), Some(-48));
        assert!(world.passable_column(5, -48, 5));
    }

    #[test]
    fn set_block_edits_in_place() {
        let mut world = World::new(774);
        let sections: Vec<Option<Section>> = (0..24).map(|_| None).collect();
        world.chunks.insert((0, 0), Column { sections });
        world.set_block(2, -60, 3, 1);
        assert!(world.is_solid(2, -60, 3));
        world.set_block(2, -60, 3, 0);
        assert!(!world.is_solid(2, -60, 3));
    }
}
