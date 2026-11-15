use padpilot_icon::{ICON_ID, res_file};

/// A minimal .ico with two fake images.
fn ico() -> Vec<u8> {
    let images: [&[u8]; 2] = [b"first image!", b"second"];
    let mut out = vec![0, 0, 1, 0, 2, 0];
    let mut offset = 6 + 16 * 2;
    for (i, image) in images.iter().enumerate() {
        let side = [16u8, 32][i];
        out.extend([side, side, 0, 0, 1, 0, 32, 0]);
        out.extend((image.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += image.len();
    }
    for image in images {
        out.extend(image);
    }
    out
}

/// (type, id, data) of every resource in a .res file.
fn resources(res: &[u8]) -> Vec<(u16, u16, Vec<u8>)> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < res.len() {
        let size = u32::from_le_bytes(res[at..at + 4].try_into().unwrap()) as usize;
        let header = u32::from_le_bytes(res[at + 4..at + 8].try_into().unwrap()) as usize;
        let word = |offset: usize| {
            u16::from_le_bytes(res[at + offset..at + offset + 2].try_into().unwrap())
        };
        assert_eq!(
            (word(8), word(12)),
            (0xFFFF, 0xFFFF),
            "numbered type and id"
        );
        found.push((
            word(10),
            word(14),
            res[at + header..at + header + size].to_vec(),
        ));
        at = (at + header + size).next_multiple_of(4);
    }
    found
}

#[test]
fn every_image_becomes_an_icon_resource_and_the_group_lists_them() {
    let res = res_file(&ico()).unwrap();
    let found = resources(&res);
    assert_eq!(found.len(), 4, "empty entry, two icons, one group");
    assert_eq!((found[0].0, found[0].2.len()), (0, 0));
    assert_eq!(
        (found[1].0, found[1].1, &found[1].2[..]),
        (3, 1, &b"first image!"[..])
    );
    assert_eq!(
        (found[2].0, found[2].1, &found[2].2[..]),
        (3, 2, &b"second"[..])
    );
    let (kind, id, group) = &found[3];
    assert_eq!((*kind, *id), (14, ICON_ID));
    assert_eq!(&group[..6], &[0, 0, 1, 0, 2, 0]);
    // Each 14-byte group entry: the .ico entry's first 12 bytes, then the id.
    assert_eq!(&group[6..18], &ico()[6..18]);
    assert_eq!(&group[18..20], &1u16.to_le_bytes());
    assert_eq!(&group[20 + 12..20 + 14], &2u16.to_le_bytes());
}

#[test]
fn something_that_is_not_an_ico_is_refused() {
    assert_eq!(res_file(b"PNG not an icon"), None);
    assert_eq!(res_file(&ico()[..30]), None);
}
