//! Entry texts that link objects, for the tests of the links an object holds.

/// The `.rito` text of object `entry` (class 2), which holds the first of `links` as a `link`
/// and every one of them in a list of links.
pub fn linking(entry: u32, links: &[u32]) -> String {
    let one: String = links
        .iter()
        .take(1)
        .map(|l| format!("        0x0000abcd: link = {l:#010x}\n"))
        .collect();
    let list: String = links
        .iter()
        .map(|l| format!("            {l:#010x}\n"))
        .collect();
    format!(
        "#PROP_text\ntype: string = \"PROP\"\nversion: u32 = 3\nlinked: list[string] = {{}}\n\
         entries: map[hash,embed] = {{\n    {entry:#010x} = 0x00000002 {{\n{one}        \
         0x0000abce: list[link] = {{\n{list}        }}\n    }}\n}}\n"
    )
}
