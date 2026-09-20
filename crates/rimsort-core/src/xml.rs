//! Tiny forgiving XML reader/writer. Mod XML in the wild is often malformed (unclosed tags,
//! stray `<`, HTML entities), so this never fails: it returns whatever tree it could build.

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    /// Concatenated direct text, untrimmed.
    pub text: String,
    pub children: Vec<Node>,
    /// Attribute names on the opening tag (values are not kept).
    pub attrs: Vec<String>,
}

impl Node {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// First child by name, ASCII case-insensitive.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn child_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.children
            .iter_mut()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children
            .iter()
            .filter(move |c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn trimmed(&self) -> &str {
        self.text.trim()
    }

    /// Trimmed text of the named child, if present and non-empty.
    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name)
            .map(Node::trimmed)
            .filter(|s| !s.is_empty())
    }

    /// Like `li_texts`, but skips `<li>` carrying attributes other than `IgnoreIfNoMatchingField`.
    /// RimSort's parser silently drops such entries (e.g. `MayRequire`) from load-order lists;
    /// we mirror that so sort results match.
    pub fn li_texts_rimsort(&self) -> Vec<String> {
        self.children_named("li")
            .filter(|c| c.attrs.is_empty() || c.attrs == ["IgnoreIfNoMatchingField"])
            .map(|c| c.trimmed())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Trimmed, non-empty text of `<li>` children.
    pub fn li_texts(&self) -> Vec<String> {
        self.children_named("li")
            .map(|c| c.trimmed())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }
}

/// Decode bytes of unknown encoding (BOM sniffing, UTF-8, else Windows-1252) to a string.
pub fn decode_bytes(bytes: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc
            .decode_without_bom_handling(&bytes[bom..])
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn unescape(s: &str, out: &mut String) {
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest.find(';').filter(|&e| e <= 10).and_then(|e| {
            let ent = &rest[1..e];
            let ch = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => ent.strip_prefix('#').and_then(|n| {
                    match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok(),
                        None => n.parse().ok(),
                    }
                    .and_then(char::from_u32)
                }),
            };
            ch.map(|c| (c, e + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
}

/// Parse to a tree. Returns a synthetic root whose children are the top-level elements.
pub fn parse(src: &str) -> Node {
    let mut stack = vec![Node::new("#root")];
    let mut i = 0;
    let b = src.as_bytes();

    while i < b.len() {
        if b[i] != b'<' {
            let end = src[i..].find('<').map_or(b.len(), |p| i + p);
            unescape(&src[i..end], &mut stack.last_mut().unwrap().text);
            i = end;
            continue;
        }
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            i = rest.find("-->").map_or(b.len(), |p| i + p + 3);
        } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").unwrap_or(body.len());
            stack.last_mut().unwrap().text.push_str(&body[..end]);
            i += 9 + end + 3;
        } else if rest.starts_with("<?") {
            i = rest.find("?>").map_or(b.len(), |p| i + p + 2);
        } else if rest.starts_with("<!") {
            i = rest.find('>').map_or(b.len(), |p| i + p + 1);
        } else if let Some(close) = rest.strip_prefix("</") {
            let end = close.find('>').unwrap_or(close.len());
            let name = close[..end].trim();
            // Pop to the matching open tag; ignore stray closers.
            if let Some(pos) = stack
                .iter()
                .rposition(|n| n.name == name)
                .filter(|&p| p > 0)
            {
                while stack.len() > pos {
                    let n = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(n);
                }
            }
            i += 2 + end + 1;
        } else if rest[1..].starts_with(|c: char| c.is_alphabetic() || c == '_') {
            // Open tag: read name, then skip attributes honoring quotes.
            let name_end = rest[1..]
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .map_or(rest.len(), |p| p + 1);
            let name = &rest[1..name_end];
            let (mut j, mut quote, mut self_closing) = (name_end, None, false);
            let rb = rest.as_bytes();
            while j < rb.len() {
                match (quote, rb[j]) {
                    (Some(q), c) if c == q => quote = None,
                    (Some(_), _) => {}
                    (None, b'"' | b'\'') => quote = Some(rb[j]),
                    (None, b'>') => {
                        self_closing = rb[j - 1] == b'/';
                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            let mut node = Node::new(name);
            node.attrs = attr_names(&rest[name_end..j]);
            if self_closing {
                stack.last_mut().unwrap().children.push(node);
            } else {
                stack.push(node);
            }
            i += j + 1;
        } else {
            // Bare `<` in text.
            stack.last_mut().unwrap().text.push('<');
            i += 1;
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(n);
    }
    stack.pop().unwrap()
}

/// Names of `name="value"` attributes in the text between a tag name and its `>`.
fn attr_names(s: &str) -> Vec<String> {
    let (mut out, mut quote, mut cur) = (Vec::new(), None, String::new());
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '=') => {
                let n = cur.trim().to_owned();
                if !n.is_empty() {
                    out.push(n);
                }
                cur.clear();
            }
            (None, c) if c.is_whitespace() => cur.clear(),
            (None, '/') => {}
            (None, c) => cur.push(c),
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Serialize with 2-space indent (RimWorld style). Leaf text nodes stay on one line.
pub fn write(node: &Node) -> String {
    fn go(n: &Node, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        if n.children.is_empty() {
            let t = n.trimmed();
            if t.is_empty() {
                out.push_str(&format!("{pad}<{}/>\n", n.name));
            } else {
                out.push_str(&format!("{pad}<{0}>{1}</{0}>\n", n.name, escape(t)));
            }
        } else {
            out.push_str(&format!("{pad}<{}>\n", n.name));
            for c in &n.children {
                go(c, depth + 1, out);
            }
            out.push_str(&format!("{pad}</{}>\n", n.name));
        }
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    go(node, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerant_parse() {
        let root = parse(
            "\u{feff}<?xml version=\"1.0\"?><A x='1>2'><b>a &amp; b &nbsp; 1 < 2</b><c>unclosed<d/></A>",
        );
        let a = root.child("a").unwrap();
        assert_eq!(a.child("B").unwrap().trimmed(), "a & b \u{a0} 1 < 2");
        assert_eq!(a.child("c").unwrap().children[0].name, "d");
    }

    #[test]
    fn attrs_and_rimsort_li() {
        let r = parse(
            r#"<a><li MayRequire="x.y">skip</li><li>keep</li><li IgnoreIfNoMatchingField="True">also</li><li A="1" B='two words'>no</li></a>"#,
        );
        assert_eq!(r.child("a").unwrap().li_texts_rimsort(), ["keep", "also"]);
        assert_eq!(r.child("a").unwrap().li_texts().len(), 4);
    }

    #[test]
    fn roundtrip_write() {
        let root = parse("<R><v>1</v><l><li>a</li><li>b&amp;c</li></l></R>");
        let out = write(root.child("R").unwrap());
        assert!(out.contains("<li>b&amp;c</li>") && out.contains("  <v>1</v>"));
    }
}
