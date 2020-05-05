use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use moteria::{iro, iro_mmap};
use std::{path::Path, time::Duration};

const TEST_IROS: &[&str] = &[
    // r"D:\Downloads\Remako HD Graphics Mod v1.0 - Menu Avatars (menu).iro",
    // r"D:\Downloads\Enhanced Stock UI-rel2.iro",
    // r"D:\Downloads\ZomiPlayFont 0.8.iro",
    r"D:\Downloads\Remako_HD_Graphics_Mod_v1.0_-_Complete_Download_battle_menu_field_char_minigames_world_movie\Remako HD Graphics Mod v1.0 - Menu Avatars (menu).iro",
    r"D:\Downloads\Remako_HD_Graphics_Mod_v1.0_-_Complete_Download_battle_menu_field_char_minigames_world_movie\Remako HD Graphics Mod v1.0 - World Textures (world).iro",
    r"D:\Downloads\Remako_HD_Graphics_Mod_v1.0_-_Complete_Download_battle_menu_field_char_minigames_world_movie\Remako HD Graphics Mod v1.0 - Battle Textures (battle).iro",
    r"D:\Downloads\Remako_HD_Graphics_Mod_v1.0_-_Complete_Download_battle_menu_field_char_minigames_world_movie\Remako HD Graphics Mod v1.0 - Pre-Rendered Backgrounds (field + char + minigames).iro",
];
 // weird LZMA errors that I need to fix: r"C:\Users\Simon\Documents\MEGAsync Downloads\FF7 NT IRO 3rd May 2020\FF7 NT IRO.iro",

struct BlackHole;

impl std::io::Write for BlackHole {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        black_box(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

fn bench_mmap(path: &Path) {
    let mut iro = iro_mmap::open(path).unwrap();

    let len = iro.files.len();

    for idx in 0..len {
        if let Err(err) = iro.extract_to(BlackHole {}, idx) {
            println!("MMAP Error: {}", err);
        }
    }
}

fn bench_classic(path: &Path) {
    let mut iro = iro::open(path).unwrap();

    let len = iro.files.len();

    for idx in 0..len {
        if let Err(err) = iro.extract_to(BlackHole {}, idx) {
            println!("Direct Error: {}", err);
        }
    }
}

fn bench_iro(c: &mut Criterion) {
    let mut group = c.benchmark_group("Verify Zip");

    for file in TEST_IROS.iter() {
        let file = Path::new(file);
        let file_name = file.file_name().unwrap().to_string_lossy();
        let file_size = (file.metadata().unwrap().len() as f64) / 1_048_576_f64;
        let display = format!("{} / {:.2} MB", file_name, file_size);
        // let time = Duration::from_secs_f64(file_size * 1.2);
        // let group = group.measurement_time(time);

        group.bench_with_input(BenchmarkId::new("Direct", &display), &file, |b, f| b.iter(|| bench_classic(f)));
        group.bench_with_input(BenchmarkId::new("mmap", &display), &file, |b, f| b.iter(|| bench_mmap(f)));
    }

    group.finish();
}

//         .measurement_time(Duration::from_secs(10))

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .plotting_backend(criterion::PlottingBackend::Plotters)
        .confidence_level(0.7)
        .without_plots()
        .save_baseline("Base".to_string())
        ;
    targets = bench_iro
}
criterion_main!(benches);
