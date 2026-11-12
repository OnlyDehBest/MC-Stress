use std::collections::HashMap;

use crate::protocol::PayloadReader;

#[derive(Clone, Copy)]
pub struct Entity {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Lightweight tracker of nearby entities, enough to target the closest one.
/// Positions come from spawn (absolute), relative-move deltas and teleports.
#[derive(Default)]
pub struct EntityStore {
    entities: HashMap<i32, Entity>,
}

impl EntityStore {
    pub fn new() -> EntityStore {
        EntityStore::default()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn spawn(&mut self, body: &[u8]) {
        let mut reader = PayloadReader::new(body);
        let Ok(id) = reader.read_varint() else { return };
        if reader.skip(16).is_err() {
            return;
        } // objectUUID
        if reader.read_varint().is_err() {
            return;
        } // type
        let (Ok(x), Ok(y), Ok(z)) = (reader.read_f64(), reader.read_f64(), reader.read_f64()) else {
            return;
        };
        self.entities.insert(id, Entity { x, y, z });
    }

    pub fn relative_move(&mut self, body: &[u8]) {
        let mut reader = PayloadReader::new(body);
        let Ok(id) = reader.read_varint() else { return };
        let (Ok(dx), Ok(dy), Ok(dz)) = (reader.read_i16(), reader.read_i16(), reader.read_i16()) else {
            return;
        };
        if let Some(entity) = self.entities.get_mut(&id) {
            // Delta encoding: (current * 4096) units, 1/4096 block each.
            entity.x += dx as f64 / 4096.0;
            entity.y += dy as f64 / 4096.0;
            entity.z += dz as f64 / 4096.0;
        }
    }

    pub fn teleport(&mut self, body: &[u8]) {
        let mut reader = PayloadReader::new(body);
        let Ok(id) = reader.read_varint() else { return };
        // Entity Teleport always begins with the absolute f64 x/y/z; trailing
        // fields differ by version and are ignored.
        let (Ok(x), Ok(y), Ok(z)) = (reader.read_f64(), reader.read_f64(), reader.read_f64()) else {
            return;
        };
        self.entities.entry(id).and_modify(|e| {
            e.x = x;
            e.y = y;
            e.z = z;
        });
    }

    pub fn destroy(&mut self, body: &[u8]) {
        let mut reader = PayloadReader::new(body);
        let Ok(count) = reader.read_varint() else { return };
        for _ in 0..count {
            let Ok(id) = reader.read_varint() else { return };
            self.entities.remove(&id);
        }
    }

    /// Closest tracked entity to (x, y, z) within `range` blocks, if any.
    pub fn nearest(&self, x: f64, y: f64, z: f64, range: f64) -> Option<(i32, Entity)> {
        let range_sq = range * range;
        let mut best: Option<(i32, Entity, f64)> = None;
        for (id, entity) in &self.entities {
            let dx = entity.x - x;
            let dy = entity.y - y;
            let dz = entity.z - z;
            let distance = dx * dx + dy * dy + dz * dz;
            if distance <= range_sq && best.as_ref().map(|b| distance < b.2).unwrap_or(true) {
                best = Some((*id, *entity, distance));
            }
        }
        best.map(|(id, entity, _)| (id, entity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn spawn_then_nearest() {
        let mut store = EntityStore::new();
        let mut body = Vec::new();
        crate::protocol::write_varint(&mut body, 7); // entity id
        body.extend_from_slice(&[0u8; 16]); // uuid
        crate::protocol::write_varint(&mut body, 0); // type
        crate::protocol::write_f64(&mut body, 10.0);
        crate::protocol::write_f64(&mut body, 64.0);
        crate::protocol::write_f64(&mut body, 10.0);
        body.extend_from_slice(&[0u8; 32]); // trailing fields, ignored
        store.spawn(&body);
        assert_eq!(store.len(), 1);
        let (id, entity) = store.nearest(11.0, 64.0, 10.0, 5.0).unwrap();
        assert_eq!(id, 7);
        assert!((entity.x - 10.0).abs() < 1e-9);
        assert!(store.nearest(100.0, 64.0, 100.0, 5.0).is_none());
    }

    #[test]
    fn relative_move_updates() {
        let mut store = EntityStore::new();
        let mut spawn = Vec::new();
        crate::protocol::write_varint(&mut spawn, 3);
        spawn.extend_from_slice(&[0u8; 16]);
        crate::protocol::write_varint(&mut spawn, 0);
        crate::protocol::write_f64(&mut spawn, 0.0);
        crate::protocol::write_f64(&mut spawn, 64.0);
        crate::protocol::write_f64(&mut spawn, 0.0);
        store.spawn(&spawn);

        let mut mv = Vec::new();
        crate::protocol::write_varint(&mut mv, 3);
        mv.extend_from_slice(&4096i16.to_be_bytes()); // +1 block x
        mv.extend_from_slice(&0i16.to_be_bytes());
        mv.extend_from_slice(&0i16.to_be_bytes());
        mv.push(1);
        store.relative_move(&mv);

        let (_, entity) = store.nearest(0.0, 64.0, 0.0, 10.0).unwrap();
        assert!((entity.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn destroy_removes() {
        let mut store = EntityStore::new();
        let _ = concat(&[&[]]);
        let mut spawn = Vec::new();
        crate::protocol::write_varint(&mut spawn, 9);
        spawn.extend_from_slice(&[0u8; 16]);
        crate::protocol::write_varint(&mut spawn, 0);
        crate::protocol::write_f64(&mut spawn, 1.0);
        crate::protocol::write_f64(&mut spawn, 1.0);
        crate::protocol::write_f64(&mut spawn, 1.0);
        store.spawn(&spawn);
        let mut destroy = Vec::new();
        crate::protocol::write_varint(&mut destroy, 1);
        crate::protocol::write_varint(&mut destroy, 9);
        store.destroy(&destroy);
        assert_eq!(store.len(), 0);
    }
}
