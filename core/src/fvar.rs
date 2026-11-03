// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Named-instance parsing for the `fvar` table.
//!
//! `ttf-parser` exposes variation axes but not named instances, so the instance
//! records are read directly from the table bytes.

/// A named instance as stored in `fvar`, before its name IDs are resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawInstance {
    pub subfamily_name_id: u16,
    pub postscript_name_id: Option<u16>,
    pub coordinates: Vec<f32>,
}

/// Reads all named instances from raw `fvar` table bytes.
///
/// Returns `None` if the table is truncated or its header is inconsistent.
pub(crate) fn parse_instances(data: &[u8]) -> Option<Vec<RawInstance>> {
    let axes_offset = usize::from(be_u16(data, 4)?);
    let axis_count = usize::from(be_u16(data, 8)?);
    let axis_size = usize::from(be_u16(data, 10)?);
    let instance_count = usize::from(be_u16(data, 12)?);
    let instance_size = usize::from(be_u16(data, 14)?);

    let coords_len = axis_count * 4;
    // Each record is: subfamily name ID, flags, coordinates, then an optional PostScript name ID.
    if instance_size < 4 + coords_len {
        return None;
    }
    let has_postscript_id = instance_size == 4 + coords_len + 2;
    let instances_start = axes_offset + axis_count * axis_size;

    (0..instance_count)
        .map(|i| {
            let off = instances_start + i * instance_size;
            let coordinates = (0..axis_count)
                .map(|a| be_i32(data, off + 4 + a * 4).map(|raw| raw as f32 / 65536.0))
                .collect::<Option<Vec<_>>>()?;
            let postscript_name_id = if has_postscript_id {
                be_u16(data, off + 4 + coords_len).filter(|&id| id != 0xFFFF)
            } else {
                None
            };
            Some(RawInstance {
                subfamily_name_id: be_u16(data, off)?,
                postscript_name_id,
                coordinates,
            })
        })
        .collect()
}

fn be_u16(data: &[u8], off: usize) -> Option<u16> {
    let bytes = data.get(off..off + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be_i32(data: &[u8], off: usize) -> Option<i32> {
    let bytes = data.get(off..off + 4)?;
    Some(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an fvar table with `axis_count` axes (zeroed, not read here) and the given instances.
    fn build_fvar(axis_count: u16, instances: &[(u16, Option<u16>, &[f32])]) -> Vec<u8> {
        let axes_offset: u16 = 16;
        let axis_size: u16 = 20;
        let coords_len = usize::from(axis_count) * 4;
        let has_ps = instances.first().is_some_and(|(_, ps, _)| ps.is_some());
        let instance_size = 4 + coords_len + if has_ps { 2 } else { 0 };

        let mut out = Vec::new();
        out.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
        out.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
        out.extend_from_slice(&axes_offset.to_be_bytes());
        out.extend_from_slice(&2u16.to_be_bytes()); // reserved
        out.extend_from_slice(&axis_count.to_be_bytes());
        out.extend_from_slice(&axis_size.to_be_bytes());
        out.extend_from_slice(&(instances.len() as u16).to_be_bytes());
        out.extend_from_slice(&(instance_size as u16).to_be_bytes());
        out.extend(std::iter::repeat_n(
            0u8,
            usize::from(axis_count) * usize::from(axis_size),
        ));
        for (subfamily, ps, coords) in instances {
            out.extend_from_slice(&subfamily.to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes()); // flags
            for c in coords.iter() {
                out.extend_from_slice(&((c * 65536.0) as i32).to_be_bytes());
            }
            if has_ps {
                out.extend_from_slice(&ps.unwrap_or(0xFFFF).to_be_bytes());
            }
        }
        out
    }

    #[test]
    fn parses_instances_with_coordinates() {
        let data = build_fvar(
            2,
            &[(258, None, &[400.0, 100.0]), (259, None, &[700.5, 75.0])],
        );
        let parsed = parse_instances(&data).expect("valid table");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].subfamily_name_id, 258);
        assert_eq!(parsed[0].coordinates, vec![400.0, 100.0]);
        assert_eq!(parsed[1].coordinates, vec![700.5, 75.0]);
        assert_eq!(parsed[1].postscript_name_id, None);
    }

    #[test]
    fn reads_optional_postscript_name_id() {
        let data = build_fvar(1, &[(260, Some(300), &[600.0])]);
        let parsed = parse_instances(&data).expect("valid table");
        assert_eq!(parsed[0].postscript_name_id, Some(300));
    }

    #[test]
    fn rejects_truncated_table() {
        let mut data = build_fvar(1, &[(260, None, &[600.0])]);
        data.truncate(data.len() - 2);
        assert_eq!(parse_instances(&data), None);
    }

    #[test]
    fn rejects_inconsistent_instance_size() {
        let mut data = build_fvar(2, &[(260, None, &[600.0, 1.0])]);
        // Corrupt instanceSize (offset 14) to be smaller than the coordinate block.
        data[14..16].copy_from_slice(&4u16.to_be_bytes());
        assert_eq!(parse_instances(&data), None);
    }
}
