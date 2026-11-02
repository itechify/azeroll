//! Optional byte comparison against the native engine, never writes install bytes.
use benilla_mpq::Archive;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};
use wenilla_mpq_web::{extract, MpqIndex};
fn read_at(file: &mut File, at: u64, len: usize) -> Vec<u8> {
    file.seek(SeekFrom::Start(at)).unwrap();
    let mut b = vec![0; len];
    file.read_exact(&mut b).unwrap();
    b
}
#[test]
fn browser_matches_native_for_manifests_and_world_samples() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder = std::env::var_os("DATA_FOLDER")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("wow-data/Data"));
    if !folder.is_dir() {
        eprintln!("No local install; skipping byte comparison");
        return;
    }
    let mut names = std::collections::BTreeSet::<String>::new();
    for manifest in ["boot", "world"] {
        let v: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("web/{manifest}-manifest.json"))).unwrap(),
        )
        .unwrap();
        names.extend(
            v["names"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_str().unwrap().to_owned()),
        );
    }
    let mut checked = 0;
    let mut absent = 0;
    // Compare every archive's entry, including lower-priority copies, independent of mount order.
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("mpq"))
        {
            continue;
        }
        let archive = Archive::open(&path).unwrap();
        let mut file = File::open(&path).unwrap();
        let h = read_at(&mut file, 0, 32);
        let u = |i| u32::from_le_bytes(h[i..i + 4].try_into().unwrap());
        let index = MpqIndex::new(
            read_at(&mut file, u(16) as u64, u(24) as usize * 16),
            read_at(&mut file, u(20) as u64, u(28) as usize * 16),
        )
        .unwrap();
        let sector = 512usize << u16::from_le_bytes(h[14..16].try_into().unwrap());
        let mut samples = names.clone();
        if let Ok(list) = archive.read_file("(listfile)") {
            let list = String::from_utf8_lossy(&list);
            for ext in [".m2", ".blp", ".adt"] {
                samples.extend(
                    list.split([';', '\r', '\n'])
                        .filter(|n| n.to_lowercase().ends_with(ext))
                        .take(12)
                        .map(str::to_owned),
                );
            }
        }
        for name in samples {
            if name == "__index" {
                continue;
            }
            let b = index.lookup(&name);
            if b.is_empty() || b[3] & 0x2000000 != 0 {
                absent += 1;
                continue;
            }
            let native = archive.read_file(&name).unwrap();
            let raw = read_at(&mut file, b[0] as u64, b[1] as usize);
            assert_eq!(
                extract(&name, raw, b[0], b[2] as usize, b[3], sector).unwrap(),
                native,
                "{}: {name}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 600, "Insufficient local coverage: {checked}");
    eprintln!("Compared {checked} archive entries byte-for-byte; {absent} per-archive absent/tombstoned probes");
}
