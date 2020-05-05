use crate::imports::*;

#[derive(Debug, Default)]
struct ModInfo<'a> {
    name: Option<&'a str>,
    id: Option<&'a str>,
    author: Option<&'a str>,
    version: Option<&'a str>,
    description: Option<&'a str>,
    link: Option<&'a str>,
    preview_file: Option<&'a str>,
    category: Option<&'a str>,
    release_date: Option<&'a str>,
    release_notes: Option<&'a str>,

    mod_folders: Vec<Folder<'a>>,
    config_options: Vec<ConfigOption<'a>>,
}

#[derive(Debug)]
struct Folder<'a> {
    folder: &'a str,
    active_when: &'a str,
}

#[derive(Debug, Default)]
struct ConfigOption<'a> {
    type_: Option<&'a str>,
    default: bool,
    id: Option<&'a str>,
    name: Option<&'a str>,
    description: Option<&'a str>,
    options: Vec<ConfigOptionOption<'a>>,
}

#[derive(Debug)]
struct ConfigOptionOption<'a> {
    value: i32,
    name: &'a str,
    preview_file: &'a str,
}

fn parse_config_option<'a>(parent: roxmltree::Node<'a, '_>) -> Result<ConfigOption<'a>> {
    let mut opt = ConfigOption::default();

    for node in parent.children().filter(|n| n.is_element()) {
        match (node.tag_name().name(), node.text().map(|s| s.trim())) {
            ("Type", text) => opt.type_ = text,
            ("Default", text) => opt.default = text.unwrap_or("").parse::<i32>()? > 0,
            ("ID", text) => opt.id = text,
            ("Name", text) => opt.name = text,
            ("Description", text) => opt.description = text,
            (unk, _) => unimplemented!("unimplemented ConfigOption node {}", unk),
        }
        // println!("{} {:?}", node.tag_name().name(), node.text());
    }

    unimplemented!()
}

fn parse_mod_folder<'a>(node: roxmltree::Node<'a, '_>) -> Result<()> {
    unimplemented!()
}

pub fn open<P: AsRef<path::Path>>(path: P) -> Result<()> {
    let mut info = ModInfo::default();

    let string = fs::read_to_string(path.as_ref())?;
    let doc = roxmltree::Document::parse(&string)?;

    let root = doc.root_element();
    assert!(root.has_tag_name("ModInfo"));

    for node in root.children().filter(|n| n.is_element()) {
        match (node.tag_name().name(), node.text().map(|s| s.trim())) {
            ("Name", text) => info.name = text,
            ("ID", text) => info.id = text,
            ("Author", text) => info.author = text,
            ("Version", text) => info.version = text,
            ("Description", text) => info.description = text,
            ("Link", text) => info.link = text,
            ("PreviewFile", text) => info.preview_file = text,
            ("Category", text) => info.category = text,
            ("ReleaseDate", text) => info.release_date = text,
            ("ReleaseNotes", text) => info.release_notes = text,
            ("ConfigOption", _) => info.config_options.push(parse_config_option(node)?),
            ("ModFolder", _) => parse_mod_folder(node)?,
            (unk, _) => unimplemented!("unimplemented XML node {}", unk),
        }
        // println!("{} {:?}", node.tag_name().name(), node.text());
    }

    println!("{:?}", info);

    Ok(())
}
