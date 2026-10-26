use lidar_simulator::demo::{DemoOptions, run, run_headless};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = DemoOptions::default();
    let mut headless = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--headless" {
            headless = true;
            continue;
        }
        if arg == "--help" || arg == "-h" {
            println!(
                "lidar-simulator [--headless] [--source-step 0.08] [--scan-step 0.25]\n  [--noise-probability 0.02] [--noise-extent 0.12] [--frames 72]\n  [--seed 12648430] [--output assets/export/building-scan.ply]"
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
