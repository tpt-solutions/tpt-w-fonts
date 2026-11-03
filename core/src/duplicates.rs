// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Duplicate detection over the catalog, and safe resolution of duplicate groups.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::Catalog;
use crate::error::Result;
use crate::model::{FontFile, FontSource};

/// Why a set of files was grouped together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DuplicateKind {
    /// Byte-identical files (same BLAKE3 content hash).
    ExactContent,
    /// Different files that declare the same family, subfamily, version and vendor.
    SameMetadata,
}

/// A set of files that look like duplicates of one another.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DuplicateGroup {
    pub kind: DuplicateKind,
    /// The shared content hash, or the shared metadata key for [`DuplicateKind::SameMetadata`].
    pub key: String,
    pub members: Vec<PathBuf>,
}

/// Finds exact and metadata duplicates in the catalog.
///
/// Exact groups come first. A metadata group only counts files with differing content hashes,
/// so byte-identical copies are not reported twice.
pub fn find_duplicates(catalog: &Catalog) -> Vec<DuplicateGroup> {
    let mut groups = exact_groups(catalog.files());
    groups.extend(metadata_groups(catalog.files()));
    groups
}

fn exact_groups(files: &[FontFile]) -> Vec<DuplicateGroup> {
    group_by(files, |file| Some(file.content_hash.clone()))
        .into_iter()
        .map(|(hash, members)| DuplicateGroup {
            kind: DuplicateKind::ExactContent,
            key: hash,
            members,
        })
        .collect()
}

fn metadata_groups(files: &[FontFile]) -> Vec<DuplicateGroup> {
    // A file may hold several faces (a .ttc); each face contributes its own metadata key.
    let mut by_key: BTreeMap<String, Vec<&FontFile>> = BTreeMap::new();
    for file in files {
        let keys: BTreeSet<String> = file
            .faces
            .iter()
            .map(|face| {
                format!(
                    "{}|{}|{}|{}",
                    face.family,
                    face.subfamily,
                    face.version,
                    face.vendor_id.as_deref().unwrap_or("")
                )
            })
            .collect();
        for key in keys {
            by_key.entry(key).or_default().push(file);
        }
    }

    by_key
        .into_iter()
        .filter_map(|(key, candidates)| {
            let distinct_hashes: BTreeSet<&str> =
                candidates.iter().map(|f| f.content_hash.as_str()).collect();
            // Only report when there are at least two different contents to compare.
            if distinct_hashes.len() < 2 {
                return None;
            }
            let members = candidates.iter().map(|f| f.path.clone()).collect();
            Some(DuplicateGroup {
                kind: DuplicateKind::SameMetadata,
                key,
                members,
            })
        })
        .collect()
}

fn group_by<F>(files: &[FontFile], key: F) -> Vec<(String, Vec<PathBuf>)>
where
    F: Fn(&FontFile) -> Option<String>,
{
    let mut map: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for file in files {
        if let Some(k) = key(file) {
            map.entry(k).or_default().push(file.path.clone());
        }
    }
    map.into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect()
}

/// The result of resolving one duplicate group.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    /// Files that were (or, in a dry run, would be) sent to the trash.
    pub trashed: Vec<PathBuf>,
    /// Files left alone because they are system fonts, which are read-only in v1.
    pub skipped_system: Vec<PathBuf>,
}

/// Keeps `keep` and sends the other members of `group` to the OS trash.
///
/// Nothing is permanently deleted. System-source files are never touched. With `dry_run`, the
/// plan is returned without changing anything.
pub fn keep_one(
    catalog: &Catalog,
    group: &DuplicateGroup,
    keep: &Path,
    dry_run: bool,
) -> Result<Resolution> {
    let mut resolution = Resolution::default();
    for member in &group.members {
        if member == keep {
            continue;
        }
        let is_system = catalog
            .files()
            .iter()
            .find(|file| file.path == *member)
            .is_some_and(|file| file.source == FontSource::System);
        if is_system {
            resolution.skipped_system.push(member.clone());
        } else {
            resolution.trashed.push(member.clone());
        }
    }

    if !dry_run && !resolution.trashed.is_empty() {
        trash::delete_all(&resolution.trashed).map_err(|e| crate::FontError::Io {
            path: resolution.trashed[0].clone(),
            source: std::io::Error::other(e),
        })?;
    }
    Ok(resolution)
}

/// Field-by-field differences between two files' primary faces, for a side-by-side view.
///
/// Each entry is `(field, left, right)` and only differing fields are included.
pub fn metadata_diff(left: &FontFile, right: &FontFile) -> Vec<(&'static str, String, String)> {
    let mut rows = vec![
        (
            "format",
            format!("{:?}", left.format),
            format!("{:?}", right.format),
        ),
        (
            "size (bytes)",
            left.size.to_string(),
            right.size.to_string(),
        ),
        (
            "faces",
            left.faces.len().to_string(),
            right.faces.len().to_string(),
        ),
    ];
    if let (Some(a), Some(b)) = (left.faces.first(), right.faces.first()) {
        rows.extend([
            ("family", a.family.clone(), b.family.clone()),
            ("subfamily", a.subfamily.clone(), b.subfamily.clone()),
            ("full name", a.full_name.clone(), b.full_name.clone()),
            ("version", a.version.clone(), b.version.clone()),
            (
                "vendor",
                a.vendor_id.clone().unwrap_or_default(),
                b.vendor_id.clone().unwrap_or_default(),
            ),
            (
                "embedding",
                format!("{:?}", a.embedding),
                format!("{:?}", b.embedding),
            ),
            (
                "units per em",
                a.units_per_em.to_string(),
                b.units_per_em.to_string(),
            ),
        ]);
    }
    rows.into_iter().filter(|(_, l, r)| l != r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EmbeddingRights, FontFace, FontFormat};

    fn face(family: &str, version: &str, vendor: Option<&str>) -> FontFace {
        FontFace {
            index: 0,
            family: family.into(),
            subfamily: "Regular".into(),
            full_name: format!("{family} Regular"),
            postscript_name: None,
            version: version.into(),
            vendor_id: vendor.map(str::to_string),
            embedding: EmbeddingRights::Installable,
            units_per_em: 1000,
            is_variable: false,
            axes: vec![],
            named_instances: vec![],
        }
    }

    fn file(path: &str, hash: &str, source: FontSource, faces: Vec<FontFace>) -> FontFile {
        FontFile {
            path: PathBuf::from(path),
            source,
            format: FontFormat::Ttf,
            size: 100,
            modified_ms: 0,
            content_hash: hash.into(),
            faces,
        }
    }

    fn catalog(files: Vec<FontFile>) -> Catalog {
        Catalog::from_files(files)
    }

    #[test]
    fn groups_byte_identical_files() {
        let c = catalog(vec![
            file(
                "a/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", None)],
            ),
            file(
                "b/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", None)],
            ),
            file(
                "c/Bar.ttf",
                "h2",
                FontSource::User,
                vec![face("Bar", "1.0", None)],
            ),
        ]);
        let groups = find_duplicates(&c);
        let exact: Vec<_> = groups
            .iter()
            .filter(|g| g.kind == DuplicateKind::ExactContent)
            .collect();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].members.len(), 2);
    }

    #[test]
    fn metadata_match_with_different_bytes_is_reported() {
        let c = catalog(vec![
            file(
                "a/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", Some("TPT"))],
            ),
            file(
                "b/Foo-v2.ttf",
                "h2",
                FontSource::User,
                vec![face("Foo", "1.0", Some("TPT"))],
            ),
        ]);
        let groups = find_duplicates(&c);
        let meta: Vec<_> = groups
            .iter()
            .filter(|g| g.kind == DuplicateKind::SameMetadata)
            .collect();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].members.len(), 2);
    }

    #[test]
    fn identical_bytes_alone_do_not_create_metadata_group() {
        let c = catalog(vec![
            file(
                "a/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", None)],
            ),
            file(
                "b/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", None)],
            ),
        ]);
        let groups = find_duplicates(&c);
        assert!(groups.iter().all(|g| g.kind == DuplicateKind::ExactContent));
    }

    #[test]
    fn different_vendor_is_not_a_metadata_duplicate() {
        let c = catalog(vec![
            file(
                "a/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1.0", Some("AAAA"))],
            ),
            file(
                "b/Foo.ttf",
                "h2",
                FontSource::User,
                vec![face("Foo", "1.0", Some("BBBB"))],
            ),
        ]);
        assert!(find_duplicates(&c).is_empty());
    }

    #[test]
    fn keep_one_plans_removal_of_others_and_skips_system_fonts() {
        let c = catalog(vec![
            file(
                "u/Foo.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1", None)],
            ),
            file(
                "u/Foo-copy.ttf",
                "h1",
                FontSource::User,
                vec![face("Foo", "1", None)],
            ),
            file(
                "sys/Foo.ttf",
                "h1",
                FontSource::System,
                vec![face("Foo", "1", None)],
            ),
        ]);
        let group = find_duplicates(&c).remove(0);
        let plan = keep_one(&c, &group, Path::new("u/Foo.ttf"), true).expect("dry run");

        assert_eq!(plan.trashed, vec![PathBuf::from("u/Foo-copy.ttf")]);
        assert_eq!(plan.skipped_system, vec![PathBuf::from("sys/Foo.ttf")]);
    }

    #[test]
    fn metadata_diff_lists_only_differing_fields() {
        let a = file(
            "a.ttf",
            "h1",
            FontSource::User,
            vec![face("Foo", "1.0", Some("TPT"))],
        );
        let b = file(
            "b.ttf",
            "h2",
            FontSource::User,
            vec![face("Foo", "2.0", Some("TPT"))],
        );
        let diff = metadata_diff(&a, &b);
        assert_eq!(
            diff,
            vec![("version", "1.0".to_string(), "2.0".to_string())]
        );
    }
}
