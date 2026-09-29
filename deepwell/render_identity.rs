// Included by build.rs and its integration test. Keep the input list shared with Cargo watches.
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

const DIRECTORIES: [&str; 5] = [
    "src",
    "vendor",
    "wikidot-forms",
    "relation-impl-derive",
    "misc",
];
const FILES: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "render_identity.rs",
    "config.example.toml",
    "askama.toml",
];
const IGNORED_DIRECTORIES: [&str; 6] =
    ["tests", "benches", "examples", "docs", "target", ".git"];

pub fn input_roots(backend_root: &Path, locales_root: &Path) -> Vec<PathBuf> {
    DIRECTORIES
        .iter()
        .chain(FILES.iter())
        .map(|name| backend_root.join(name))
        .chain(std::iter::once(locales_root.to_path_buf()))
        .collect()
}

// Coarse on purpose: even a backend API change invalidates compiled pages, while
// docs and frontend-only changes do not. No Git revision or filesystem metadata enters the ID.
pub fn fingerprint(backend_root: &Path, locales_root: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    for name in DIRECTORIES {
        let root = backend_root.join(name);
        require_kind(&root, true)?;
        hash_directory(&mut hash, &root, &format!("backend/{name}"))?;
    }
    for name in FILES {
        let path = backend_root.join(name);
        require_kind(&path, false)?;
        hash_file(&mut hash, &path, &format!("backend/{name}"))?;
    }
    require_kind(locales_root, true)?;
    hash_directory(&mut hash, locales_root, "locales")?;
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(result)
}

fn require_kind(path: &Path, directory: bool) -> io::Result<()> {
    let kind = fs::symlink_metadata(path)
        .map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", path.display()))
        })?
        .file_type();
    if (directory && kind.is_dir()) || (!directory && kind.is_file()) {
        return Ok(());
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        format!("required render input has wrong type: {}", path.display()),
    ))
}

fn hash_directory(hash: &mut Sha256, root: &Path, logical_root: &str) -> io::Result<()> {
    let mut entries = fs::read_dir(root)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("non-UTF-8 render input: {}", entry.path().display()),
            )
        })?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if !IGNORED_DIRECTORIES.contains(&name) && !name.starts_with(".git") {
                hash_directory(hash, &path, &format!("{logical_root}/{name}"))?;
            }
        } else if kind.is_file() || (kind.is_symlink() && fs::metadata(&path)?.is_file())
        {
            if !name.ends_with(".md")
                && name != ".cargo_vcs_info.json"
                && !name.starts_with(".git")
            {
                hash_file(hash, &path, &format!("{logical_root}/{name}"))?;
            }
        } else {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("unsupported render input: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn hash_file(hash: &mut Sha256, path: &Path, logical_path: &str) -> io::Result<()> {
    let bytes = fs::read(path)?;
    let logical_path = logical_path.as_bytes();
    hash.update((logical_path.len() as u64).to_le_bytes());
    hash.update(logical_path);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes);
    Ok(())
}
