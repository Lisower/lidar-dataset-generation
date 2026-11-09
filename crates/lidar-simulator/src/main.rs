use lidar_simulator::demo::{DemoOptions, run, run_headless};
use lidar_simulator::trajectory::TrajectoryKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = DemoOptions::default();
    let mut headless = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--headless" {
            headless = true;
            continue;
        }
        if arg == "--fixed-fov" {
            options.auto_fov = false;
            continue;
        }
        if arg == "--list" {
            let selector = lidar_simulator::demo::preset_selector();
            let store = generation_core::presets::PresetStore::discover_all(&selector.presets_root);
            for preset in &store.presets {
                println!("{} · {}", preset.id, preset.name);
            }
            return Ok(());
        }
        if arg == "--help" || arg == "-h" {
            println!(
                "lidar-simulator [--headless] [--list] [--preset buildings/cabin]\n  [--trajectory orbit|line] [--fixed-fov] [--source-step 0.3]\n  [--scan-step 0.25] [--noise-probability 0.02] [--noise-extent 0.12]\n  [--frames 24] [--seed 12648430] [--output assets/export/scan.xyz]\nШаг сканера задаётся на плоскости в 10 м от камеры. Ctrl+S: выбор пресетов."
            );
            return Ok(());
        }
        if ![
            "--source-step",
            "--scan-step",
            "--noise-probability",
            "--noise-extent",
            "--frames",
            "--seed",
            "--output",
            "--preset",
            "--trajectory",
        ]
        .contains(&arg.as_str())
        {
            return Err(format!("Неизвестный аргумент: {arg}; используйте --help").into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("Нужно значение для {arg}"))?;
        match arg.as_str() {
            "--source-step" => options.source_step = value.parse()?,
            "--scan-step" => options.scanner.sample_step = value.parse()?,
            "--noise-probability" => options.scanner.noise_probability = value.parse()?,
            "--noise-extent" => options.scanner.noise_extent = value.parse()?,
            "--frames" => options.frames = value.parse()?,
            "--seed" => options.seed = value.parse::<u64>()?.into(),
            "--output" => options.output = value,
            "--preset" => options.preset = Some(value),
            "--trajectory" => {
                options.trajectory = match value.as_str() {
                    "orbit" => TrajectoryKind::Orbit,
                    "line" => TrajectoryKind::Line,
                    _ => return Err("trajectory: ожидается orbit или line".into()),
                }
            }
            _ => unreachable!(),
        }
    }
    if headless {
        run_headless(options)?;
    } else {
        run(options)?;
    }
    Ok(())
}
