//! User skins from a directory.

use std::fs;
use std::path::{Path, PathBuf};

use crate::builtin::builtin_skin;
use crate::error::SkinError;
use crate::parse::{ParsedSkin, parse_skin};
use crate::skin::SkinOrigin;

/// One entry of the result: a skin, or the file that failed with its error.
type SkinResult = Result<ParsedSkin, (PathBuf, SkinError)>;

/// Loads the user skins in `dir`: `<dir>/<id>.toml` and `<dir>/<id>/skin.toml`, sorted by id.
///
/// An id that clashes with a built-in skin (or with an earlier entry) is an error entry. A
/// missing directory gives an empty list.
pub fn load_user_skins(dir: &Path) -> Vec<SkinResult> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => return vec![Err((dir.to_path_buf(), err.into()))],
    };
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut results: Vec<(String, SkinResult)> = Vec::new();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(err) => {
                results.push((String::new(), Err((dir.to_path_buf(), err.into()))));
                continue;
            }
        };
        if path.is_dir() {
            let file = path.join("skin.toml");
            if let (Some(name), true) = (path.file_name(), file.is_file()) {
                found.push((name.to_string_lossy().into_owned(), file));
            }
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
            && let Some(stem) = path.file_stem()
        {
            found.push((stem.to_string_lossy().into_owned(), path));
        }
    }
    found.sort();
    let mut seen: Vec<String> = Vec::new();
    for (id, path) in found {
        let result = if builtin_skin(&id).is_some() || seen.contains(&id) {
            Err((path, SkinError::DuplicateId(id.clone())))
        } else {
            load_one(&id, &path)
        };
        seen.push(id.clone());
        results.push((id, result));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results.into_iter().map(|(_, r)| r).collect()
}

fn load_one(id: &str, path: &Path) -> SkinResult {
    let text = fs::read_to_string(path).map_err(|e| (path.to_path_buf(), e.into()))?;
    let mut parsed = parse_skin(id, &text).map_err(|e| (path.to_path_buf(), e))?;
    parsed.skin.origin = SkinOrigin::File(path.to_path_buf());
    Ok(parsed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    const SKIN: &str = "[skin]\nname = \"Mine\"\n";

    #[test]
    fn missing_dir_is_empty() {
        let dir = tempdir().unwrap();
        assert!(load_user_skins(&dir.path().join("nope")).is_empty());
    }

    #[test]
    fn loads_files_and_folders_sorted() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("zeta.toml"), SKIN).unwrap();
        fs::create_dir(dir.path().join("alpha")).unwrap();
        fs::write(dir.path().join("alpha/skin.toml"), SKIN).unwrap();
        fs::write(dir.path().join("notes.txt"), "x").unwrap();
        fs::create_dir(dir.path().join("empty")).unwrap();
        let skins = load_user_skins(dir.path());
        let ids: Vec<_> = skins
            .iter()
            .map(|r| r.as_ref().unwrap().skin.id.clone())
            .collect();
        assert_eq!(ids, ["alpha", "zeta"]);
        let first = skins[0].as_ref().unwrap();
        assert_eq!(
            first.skin.origin,
            SkinOrigin::File(dir.path().join("alpha/skin.toml"))
        );
    }

    #[test]
    fn builtin_clash_and_bad_files_are_errors() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("silver.toml"), SKIN).unwrap();
        fs::write(dir.path().join("bad.toml"), "[skin").unwrap();
        let skins = load_user_skins(dir.path());
        assert_eq!(skins.len(), 2);
        assert!(matches!(&skins[0], Err((_, SkinError::Toml(_)))));
        assert!(matches!(&skins[1], Err((_, SkinError::DuplicateId(id))) if id == "silver"));
    }

    #[test]
    fn file_and_folder_with_same_id_clash() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.toml"), SKIN).unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("a/skin.toml"), SKIN).unwrap();
        let skins = load_user_skins(dir.path());
        assert_eq!(skins.len(), 2);
        assert!(
            skins
                .iter()
                .any(|r| matches!(r, Err((_, SkinError::DuplicateId(_)))))
        );
    }
}
