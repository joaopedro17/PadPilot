//! Build helper: puts an `.ico` file into a Windows program as its icon,
//! without a resource compiler. It writes a compiled resource file (`.res`)
//! and hands it to the linker, which accepts those directly.
//!
//! Use from a `build.rs`: `padpilot_icon::embed("../../assets/padpilot.ico")`.

use std::path::{Path, PathBuf};

/// The icon's resource number; windows load it with this id.
pub const ICON_ID: u16 = 1;

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// Moveable, discardable (and pure, for the group), as resource compilers set.
const ICON_FLAGS: u16 = 0x1010;
const GROUP_FLAGS: u16 = 0x1030;

/// Embeds `ico` (relative to the crate) when building for Windows.
pub fn embed(ico: &str) {
    let ico = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join(ico);
    println!("cargo:rerun-if-changed={}", ico.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let bytes = std::fs::read(&ico)
        .unwrap_or_else(|err| panic!("cannot read the icon {}: {err}", ico.display()));
    let resource =
        res_file(&bytes).unwrap_or_else(|| panic!("{} is not a valid .ico file", ico.display()));
    let out =
        PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo")).join("icon.res");
    std::fs::write(&out, resource).expect("cannot write the icon resource");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// The images of an `.ico` file as a compiled resource file: one icon
/// resource per image, and a group that lists them under [`ICON_ID`].
pub fn res_file(ico: &[u8]) -> Option<Vec<u8>> {
    let word = |at: usize| Some(u16::from_le_bytes(ico.get(at..at + 2)?.try_into().ok()?));
    let dword = |at: usize| Some(u32::from_le_bytes(ico.get(at..at + 4)?.try_into().ok()?));
    if word(0)? != 0 || word(2)? != 1 {
        return None;
    }
    let count = word(4)?;
    // A resource file starts with an empty entry.
    let mut out = resource(0, 0, 0, &[]);
    let mut group = Vec::new();
    for value in [0, 1, count] {
        group.extend(value.to_le_bytes());
    }
    for index in 0..count {
        let entry = 6 + 16 * usize::from(index);
        let size = dword(entry + 8)? as usize;
        let offset = dword(entry + 12)? as usize;
        let image = ico.get(offset..offset + size)?;
        let id = index + 1;
        out.extend(resource(RT_ICON, id, ICON_FLAGS, image));
        // The group entry is the file's entry with the offset replaced by the id.
        group.extend(ico.get(entry..entry + 12)?);
        group.extend(id.to_le_bytes());
    }
    out.extend(resource(RT_GROUP_ICON, ICON_ID, GROUP_FLAGS, &group));
    Some(out)
}

/// One resource: a header naming its type and id by number, then the data,
/// padded to four bytes.
fn resource(kind: u16, id: u16, flags: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(32u32.to_le_bytes()); // header size
    out.extend(
        [0xFFFF, kind, 0xFFFF, id]
            .iter()
            .flat_map(|value: &u16| value.to_le_bytes()),
    );
    out.extend(0u32.to_le_bytes()); // data version
    out.extend(flags.to_le_bytes());
    out.extend(0u16.to_le_bytes()); // language: neutral
    out.extend(0u32.to_le_bytes()); // version
    out.extend(0u32.to_le_bytes()); // characteristics
    out.extend(data);
    out.resize(out.len().next_multiple_of(4), 0);
    out
}
