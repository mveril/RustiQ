use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::{eri_cache::is_fingerprint, Sha256Digest};

const ADJECTIVES: &[&str] = &[
    "able", "agile", "amber", "ample", "ardent", "azure", "balanced", "benign", "blue", "bold",
    "bright", "brisk", "calm", "candid", "careful", "clear", "clever", "coherent", "cool",
    "cosmic", "crisp", "curious", "deft", "eager", "exact", "fair", "firm", "fluent", "fresh",
    "gentle", "golden", "graceful", "grand", "green", "happy", "harmonic", "humble", "keen",
    "kind", "light", "lively", "lucid", "mellow", "misty", "neat", "nimble", "noble", "patient",
    "pale", "peaceful", "precise", "quiet", "radiant", "rapid", "ready", "refined", "robust",
    "serene", "sharp", "silent", "silver", "simple", "smooth", "steady", "still", "subtle",
    "sunny", "swift", "tender", "tidy", "tranquil", "vivid", "warm", "wise",
];
const SCIENTIFIC_NOUNS: &[&str] = &[
    "accelerator",
    "amplitude",
    "anion",
    "asteroid",
    "atom",
    "baryon",
    "beam",
    "boson",
    "catalyst",
    "cation",
    "comet",
    "condensate",
    "cosmos",
    "crystal",
    "detector",
    "dipole",
    "electron",
    "fermion",
    "field",
    "flux",
    "galaxy",
    "hadron",
    "ion",
    "isotope",
    "lattice",
    "lepton",
    "magneton",
    "molecule",
    "nebula",
    "neutrino",
    "neutron",
    "nucleus",
    "orbital",
    "particle",
    "phonon",
    "photon",
    "plasma",
    "planet",
    "pluto",
    "positron",
    "proton",
    "pulsar",
    "quanta",
    "quark",
    "reagent",
    "resonance",
    "spectrum",
    "spin",
    "vector",
    "wave",
];

static NOUNS: LazyLock<String> = LazyLock::new(|| {
    let mut nouns: Vec<String> = SCIENTIFIC_NOUNS
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    for element in periodic_table::periodic_table() {
        let name = element.name.to_ascii_lowercase();
        if !nouns.contains(&name) {
            nouns.push(name);
        }
    }
    nouns.join(" ")
});

static WORDS: LazyLock<petname::Petnames<'static>> = LazyLock::new(|| petname::Petnames {
    adjectives: Cow::Borrowed(ADJECTIVES),
    adverbs: Cow::Borrowed(&[]),
    nouns: Cow::Owned(NOUNS.split_whitespace().collect()),
});

pub(super) fn regular_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache path must be a regular directory",
        ));
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    let Some((adjective, noun)) = name.split_once('-') else {
        return false;
    };
    !adjective.is_empty()
        && !noun.is_empty()
        && adjective
            .bytes()
            .chain(noun.bytes())
            .all(|byte| byte.is_ascii_lowercase())
}

fn read_mapping(path: &Path) -> io::Result<Option<String>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path)?;
        let Some(fingerprint) = target.file_name().and_then(|name| name.to_str()) else {
            return Ok(None);
        };
        // Validate the literal target; never traverse the alias.
        if is_fingerprint(fingerprint)
            && target.as_os_str() == Path::new("../eri").join(fingerprint).as_os_str()
        {
            return Ok(Some(fingerprint.to_owned()));
        }
    } else if metadata.is_file() && metadata.len() <= 65 {
        let mut contents = String::new();
        File::open(path)?.take(66).read_to_string(&mut contents)?;
        let fingerprint = contents.strip_suffix('\n').unwrap_or(&contents);
        if is_fingerprint(fingerprint) {
            return Ok(Some(fingerprint.to_owned()));
        }
    }
    Ok(None)
}

pub(super) fn mappings(root: &Path) -> io::Result<BTreeMap<String, String>> {
    let directory = root.join("names");
    match regular_directory(&directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        result => result?,
    }
    let mut mappings = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if valid_name(&name) {
            // Malformed, unreadable and concurrently removed aliases are not usable.
            if let Ok(Some(fingerprint)) = read_mapping(&entry.path()) {
                mappings.insert(name, fingerprint);
            }
        }
    }
    Ok(mappings)
}

pub(super) fn resolve(root: &Path, name: &str) -> io::Result<Option<String>> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache name must contain two lowercase words separated by a hyphen",
        ));
    }
    regular_directory(&root.join("names")).or_else(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(error)
        }
    })?;
    match read_mapping(&root.join("names").join(name)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        result => result,
    }
}

fn publish_text(path: &Path, fingerprint: &str) -> io::Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().expect("alias has parent"))?;
    writeln!(temporary, "{fingerprint}")?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    Ok(())
}

fn publish(path: &Path, fingerprint: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        match std::os::unix::fs::symlink(Path::new("../eri").join(fingerprint), path) {
            Ok(()) => return Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
                ) => {}
            Err(error) => return Err(error),
        }
    }
    publish_text(path, fingerprint)
}

pub(super) fn assign(
    root: &Path,
    fingerprint: &str,
    existing: &BTreeMap<String, String>,
) -> io::Result<String> {
    if let Some((name, _)) = existing
        .iter()
        .find(|(_, value)| value.as_str() == fingerprint)
    {
        return Ok(name.clone());
    }
    let digest: Sha256Digest = format!("sha256:{fingerprint}")
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let mut rng = ChaCha8Rng::from_seed(digest.as_ref().try_into().expect("SHA-256 has 32 bytes"));
    let directory = root.join("names");
    fs::create_dir_all(&directory)?;
    regular_directory(&directory)?;
    for name in WORDS.namer(2, "-").iter(&mut rng).take(256) {
        let path = directory.join(&name);
        match publish(&path, fingerprint) {
            Ok(()) => return Ok(name),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if read_mapping(&path).ok().flatten().as_deref() == Some(fingerprint) {
                    return Ok(name);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve a cache name after 256 attempts",
    ))
}

pub(super) fn remove_alias(root: &Path, name: &str) -> io::Result<()> {
    regular_directory(&root.join("names"))?;
    let path: PathBuf = root.join("names").join(name);
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::EriCache;

    fn entry(root: &Path, byte: char) -> String {
        let fingerprint = byte.to_string().repeat(64);
        fs::create_dir_all(root.join("eri").join(&fingerprint)).unwrap();
        fingerprint
    }

    #[test]
    fn vocabulary_contains_every_element_and_only_unique_nouns() {
        let nouns: std::collections::BTreeSet<_> = NOUNS.split_whitespace().collect();
        assert_eq!(nouns.len(), NOUNS.split_whitespace().count());
        assert!(nouns.contains("quanta"));
        assert!(!nouns.contains("quantum"));
        assert_eq!(periodic_table::periodic_table().len(), 118);
        for element in periodic_table::periodic_table() {
            assert!(nouns.contains(element.name.to_ascii_lowercase().as_str()));
        }
        let mut rng = ChaCha8Rng::from_seed([0; 32]);
        for name in WORDS.namer(2, "-").iter(&mut rng).take(512) {
            assert!(valid_name(&name));
            assert!(nouns.contains(name.split_once('-').unwrap().1));
        }
    }

    #[test]
    fn existing_names_survive_vocabulary_changes_and_listing_is_read_only() {
        let root = tempfile::tempdir().unwrap();
        let fingerprint = entry(root.path(), 'a');
        let cache = EriCache::new(root.path());
        assert!(cache.entries().unwrap()[0].name.is_none());
        assert!(!root.path().join("names").exists());
        fs::create_dir(root.path().join("names")).unwrap();
        // A persisted word need not remain in the generator's current vocabulary.
        publish_text(&root.path().join("names/ancient-aether"), &fingerprint).unwrap();
        cache.assign_missing_names().unwrap();
        let listed = cache.entries().unwrap();
        assert_eq!(listed[0].name.as_deref(), Some("ancient-aether"));
        assert!(!listed[0].verified);
        assert_eq!(
            cache.resolve_name("ancient-aether").unwrap(),
            Some(fingerprint)
        );
        assert!(cache.remove_named("ancient-aether").unwrap());
        assert!(mappings(root.path()).unwrap().is_empty());
    }

    #[test]
    fn collisions_are_not_overwritten_and_exhaustion_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("names")).unwrap();
        let first = "00".repeat(32);
        let other = "ff".repeat(32);
        let mut rng = ChaCha8Rng::from_seed([0; 32]);
        let candidates: Vec<_> = WORDS.namer(2, "-").iter(&mut rng).take(256).collect();
        publish_text(&root.path().join("names").join(&candidates[0]), &other).unwrap();
        let name = assign(root.path(), &first, &BTreeMap::new()).unwrap();
        assert_ne!(name, candidates[0]);
        assert_eq!(
            resolve(root.path(), &candidates[0]).unwrap(),
            Some(other.clone())
        );
        remove_alias(root.path(), &name).unwrap();
        for candidate in candidates {
            fs::write(
                root.path().join("names").join(candidate),
                format!("{other}\n"),
            )
            .unwrap();
        }
        assert_eq!(
            assign(root.path(), &first, &BTreeMap::new())
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn concurrent_producers_accept_the_same_winner() {
        let root = tempfile::tempdir().unwrap();
        let fingerprint = entry(root.path(), 'a');
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        assign(root.path(), &fingerprint, &BTreeMap::new()).unwrap()
                    })
                })
                .collect();
            let names: Vec<_> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            assert!(names.iter().all(|name| name == &names[0]));
        });
        assert_eq!(mappings(root.path()).unwrap().len(), 1);
    }

    #[test]
    fn multiple_aliases_resolve_and_are_cleaned_with_the_fingerprint() {
        let root = tempfile::tempdir().unwrap();
        let fingerprint = entry(root.path(), 'a');
        fs::create_dir(root.path().join("names")).unwrap();
        for name in ["quiet-xenon", "calm-photon"] {
            publish_text(&root.path().join("names").join(name), &fingerprint).unwrap();
        }
        let cache = EriCache::new(root.path());
        assert_eq!(
            cache.entries().unwrap()[0].name.as_deref(),
            Some("calm-photon")
        );
        assert_eq!(
            cache.resolve_name("quiet-xenon").unwrap(),
            Some(fingerprint.clone())
        );
        assert!(cache.remove_named(&fingerprint).unwrap());
        assert!(mappings(root.path()).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn read_only_names_directory_does_not_prevent_listing() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        entry(root.path(), 'a');
        let directory = root.path().join("names");
        fs::create_dir(&directory).unwrap();
        let permissions = fs::metadata(&directory).unwrap().permissions();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
        let cache = EriCache::new(root.path());
        let assignment = cache.assign_missing_names();
        let entries = cache.entries();
        fs::set_permissions(directory, permissions).unwrap();
        let entries = entries.unwrap();
        assert_eq!(entries.len(), 1);
        // Privileged test runners may still be able to write through mode 0555.
        if assignment.is_err() {
            assert!(entries[0].name.is_none());
        }
    }

    #[test]
    fn invalid_aliases_and_orphans_are_safe() {
        let root = tempfile::tempdir().unwrap();
        let fingerprint = entry(root.path(), 'a');
        let cache = EriCache::new(root.path());
        cache.assign_missing_names().unwrap();
        let directory = root.path().join("names");
        fs::write(directory.join("bad-target"), "../../outside").unwrap();
        fs::write(directory.join("huge-target"), "a".repeat(1000)).unwrap();
        publish_text(&directory.join("quiet-orphan"), &"b".repeat(64)).unwrap();
        assert!(cache.resolve_name("quiet-orphan").unwrap().is_none());
        assert!(cache.resolve_name("bad-target").unwrap().is_none());
        assert!(cache.remove_named("../outside").is_err());
        assert!(cache
            .remove_named("missing-photon")
            .is_ok_and(|removed| !removed));
        // Removal must work even with no manifest or payload to inspect.
        cache.remove_all().unwrap();
        assert!(!root.path().join("eri").join(fingerprint).exists());
        assert!(mappings(root.path()).unwrap().is_empty());
        assert!(directory.join("bad-target").exists());
    }

    #[cfg(unix)]
    #[test]
    fn unix_links_are_relative_and_unsafe_targets_are_never_followed() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let fingerprint = entry(root.path(), 'a');
        let cache = EriCache::new(root.path());
        cache.assign_missing_names().unwrap();
        let name = cache.entries().unwrap()[0].name.clone().unwrap();
        assert_eq!(
            fs::read_link(root.path().join("names").join(name)).unwrap(),
            Path::new("../eri").join(&fingerprint)
        );
        symlink(outside.path(), root.path().join("names/unsafe-target")).unwrap();
        assert!(cache.resolve_name("unsafe-target").unwrap().is_none());
        cache.remove_all().unwrap();
        assert!(outside.path().is_dir());

        let linked_root = tempfile::tempdir().unwrap();
        entry(linked_root.path(), 'b');
        symlink(outside.path(), linked_root.path().join("names")).unwrap();
        let cache = EriCache::new(linked_root.path());
        assert!(cache.assign_missing_names().is_err());
        assert!(cache.entries().unwrap()[0].name.is_none());
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    }
}
