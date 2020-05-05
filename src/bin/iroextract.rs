use moteria::iro_mmap as iro;

use anyhow::Result;
use std::path::Path;
use clap::clap_app;

fn try_extract<P: AsRef<Path>>(input_file: P) -> Result<()> {
    let mut iro = iro::open(input_file.as_ref())?;
    iro.extract_all()
}

fn main() {
    let matches = clap_app!(iroextract =>
        (version: "1.0")
        (author: "mona")
        (about: "IRO extractor ripped from WIP mod library")
        (@arg INPUT: +required "Sets the input file ot use")
    ).get_matches();

    let input_file = matches.value_of("INPUT").unwrap();

    if let Err(err) = try_extract(&input_file) {
        println!("Error while extracting: {}", err);
    }
}
