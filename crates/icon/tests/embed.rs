//! `embed` reads cargo's build-script variables, so this file holds a single
//! test: it runs in a process of its own, where changing them is safe.

use padpilot_icon::{embed, res_file};

#[test]
fn the_resource_is_written_only_when_building_for_windows() {
    let out = std::env::temp_dir().join(format!("padpilot-embed-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let res = out.join("icon.res");
    let manifest = env!("CARGO_MANIFEST_DIR");
    // SAFETY: the only test in this process, so nothing reads them meanwhile.
    unsafe {
        std::env::set_var("CARGO_MANIFEST_DIR", manifest);
        std::env::set_var("OUT_DIR", &out);
        std::env::set_var("CARGO_CFG_TARGET_OS", "linux");
    }
    embed("../../assets/padpilot.ico");
    assert!(!res.exists(), "nothing to embed in a Linux program");

    // SAFETY: as above.
    unsafe { std::env::set_var("CARGO_CFG_TARGET_OS", "windows") };
    embed("../../assets/padpilot.ico");
    let ico = std::fs::read(format!("{manifest}/../../assets/padpilot.ico")).unwrap();
    assert_eq!(std::fs::read(&res).unwrap(), res_file(&ico).unwrap());
    std::fs::remove_dir_all(&out).unwrap();
}
