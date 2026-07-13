use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rcheevos_dir = manifest_dir.join("..").join("vendor").join("rcheevos");

    if !rcheevos_dir.exists() {
        panic!(
            "rcheevos submodule not found at {:?}. Run: git submodule update --init --recursive",
            rcheevos_dir
        );
    }

    // Compile the rcheevos C library — only the hash and runtime subsets.
    // We skip the encrypted (3DS AES) and zip subsets to minimize deps.
    let hash_sources = [
        "src/rhash/md5.c",
        "src/rhash/hash.c",
        "src/rhash/hash_disc.c",
        "src/rhash/hash_rom.c",
        "src/rhash/cdreader.c",
    ];

    let runtime_sources = [
        "src/rc_util.c",
        "src/rcheevos/alloc.c",
        "src/rcheevos/condition.c",
        "src/rcheevos/condset.c",
        "src/rcheevos/consoleinfo.c",
        "src/rcheevos/format.c",
        "src/rcheevos/lboard.c",
        "src/rcheevos/memref.c",
        "src/rcheevos/operand.c",
        "src/rcheevos/richpresence.c",
        "src/rcheevos/runtime.c",
        "src/rcheevos/runtime_progress.c",
        "src/rcheevos/trigger.c",
        "src/rcheevos/value.c",
    ];

    let all_sources: Vec<String> = hash_sources
        .iter()
        .chain(runtime_sources.iter())
        .map(|s| rcheevos_dir.join(s).to_string_lossy().into_owned())
        .collect();

    cc::Build::new()
        .files(all_sources.iter().map(|s| s.as_str()))
        .include(rcheevos_dir.join("include"))
        .include(rcheevos_dir.join("src"))
        .define("RC_HASH_NO_ENCRYPTED", None)
        .define("RC_HASH_NO_ZIP", None)
        .define("RC_NO_CACHE", None)
        .warnings(false)
        .compile("rcheevos");

    // Tell cargo to invalidate the build when rcheevos source changes
    println!("cargo:rerun-if-changed=../vendor/rcheevos/src");
    println!("cargo:rerun-if-changed=../vendor/rcheevos/include");
}
