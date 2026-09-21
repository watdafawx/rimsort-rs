//! Mutation fuzzing: the parsers must never panic on hostile or truncated input.
//! Deterministic (seeded LCG) so failures reproduce.

use crate::{modlist_io, modsconfig::ModsConfig, rules, xml};

pub(crate) struct Rng(pub u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

/// Random byte-level damage: flip, delete, duplicate, insert markup fragments, truncate, corrupt UTF-8.
pub(crate) fn mutate(rng: &mut Rng, src: &str) -> Vec<u8> {
    const BITS: &[&str] = &[
        "<",
        ">",
        "</",
        "<!--",
        "<![CDATA[",
        "]]>",
        "&",
        "&#",
        "\"",
        "'",
        "\0",
        "\u{feff}",
        "<?xml",
        "<li>",
        "</li>",
        "&amp;",
        "&#x110000;",
        "\n\r",
    ];
    let mut b = src.as_bytes().to_vec();
    for _ in 0..1 + rng.below(6) {
        if b.is_empty() {
            break;
        }
        let i = rng.below(b.len());
        match rng.below(6) {
            0 => b[i] ^= 1 << rng.below(8),
            1 => {
                b.remove(i);
            }
            2 => {
                let j = (i + 1 + rng.below(32)).min(b.len());
                let chunk = b[i..j].to_vec();
                b.splice(i..i, chunk);
            }
            3 => {
                let frag = BITS[rng.below(BITS.len())].as_bytes().to_vec();
                b.splice(i..i, frag);
            }
            4 => b.truncate(i),
            _ => b[i] = rng.below(256) as u8,
        }
    }
    b
}

pub(crate) const ABOUT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>Fuzz &amp; Friends</name><packageId>Fuzz.Mod</packageId><author>A</author>
  <authors><li>B</li><li MayRequire="x">C</li></authors>
  <supportedVersions><li>1.5</li><li>1.6</li></supportedVersions>
  <modDependencies><li><packageId>a.b</packageId><displayName>AB</displayName><alternativePackageIds><li>c.d</li></alternativePackageIds></li></modDependencies>
  <modDependenciesByVersion><v1.6><li><packageId>e.f</packageId></li></v1.6></modDependenciesByVersion>
  <loadAfter><li>x.y</li></loadAfter><loadBeforeByVersion><v1.6><li>z.w</li></v1.6></loadBeforeByVersion>
  <description><![CDATA[Some <b>bold</b> text & more]]></description>
</ModMetaData>"#;
const CONFIG: &str = "<?xml version=\"1.0\"?><ModsConfigData><version>1.6.1 rev1</version><activeMods><li>a.a</li><li>B.b_steam</li></activeMods><knownExpansions><li>ludeon.rimworld.royalty</li></knownExpansions></ModsConfigData>";
const RULES: &str = r#"{"timestamp":1,"rules":{"a.b":{"loadAfter":{"c.d":{"name":["x"],"comment":"c"}},"loadTop":{"value":true}}}}"#;
const LIST_JSON: &str = r#"{"version":"1.6","activeMods":["a.b","c.d"],"knownExpansions":[]}"#;
const REPORT: &str =
    "Created\nTotal # of mods: 2\n\nHarmony [brrainz.harmony][https://x.y/z]\nMod [a.b][No url]\n";

#[test]
fn parsers_never_panic_on_mutated_input() {
    let mut rng = Rng(0xC0FFEE);
    type Target = (&'static str, Box<dyn Fn(&str)>);
    let targets: Vec<Target> = vec![
        (CONFIG, Box::new(|s| drop(ModsConfig::parse(s)))),
        (CONFIG, Box::new(|s| drop(modlist_io::parse_list(s)))),
        (RULES, Box::new(|s| drop(rules::ExternalRules::parse(s)))),
        (LIST_JSON, Box::new(|s| drop(modlist_io::parse_list(s)))),
        (REPORT, Box::new(|s| drop(modlist_io::parse_list(s)))),
        (ABOUT, Box::new(|s| drop(xml::write(&xml::parse(s))))),
    ];
    for _ in 0..4000 {
        for (base, f) in &targets {
            f(&xml::decode_bytes(&mutate(&mut rng, base)));
        }
        let _ = rules::parse_replacements(&mutate(&mut rng, RULES));
    }
}

#[test]
fn deeply_nested_and_huge_inputs_are_survivable() {
    // Run on a small stack to prove nothing recurses per nesting level.
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let deep = "<a>".repeat(50_000) + &"</a>".repeat(50_000);
            let tree = xml::parse(&deep);
            let _ = xml::write(&tree);
            drop(tree);
            let wide = format!("<r>{}</r>", "<li>x</li>".repeat(200_000));
            assert_eq!(
                xml::parse(&wide).child("r").unwrap().children.len(),
                200_000
            );
            let _ = xml::parse(&"<".repeat(100_000));
            let _ = xml::parse(&"&#".repeat(100_000));
        })
        .unwrap()
        .join()
        .expect("parser recursed on hostile input");
}
