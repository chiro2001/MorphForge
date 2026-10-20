use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use morphforge_app::{
    recipe,
    server::{serve, ServerOptions},
};
use std::path::PathBuf;
#[derive(Parser)]
#[command(name = "morphforge", version, about = "独立体素艺术工作台与AI Harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        data: Option<PathBuf>,
        #[arg(long, default_value_t = 8766)]
        port: u16,
        #[arg(long)]
        env_file: Option<PathBuf>,
        #[arg(long)]
        no_capture: bool,
        #[arg(long)]
        new_instance_if_busy: bool,
    },
    #[command(hide = true)]
    RecipeWorker,
    Check {
        file: PathBuf,
    },
    Recipe {
        file: PathBuf,
        #[arg(long)]
        data: Option<PathBuf>,
        #[arg(long, default_value_t = 1)]
        seed: u32,
    },
    Benchmark {
        #[arg(long, default_value_t = 16384)]
        count: usize,
    },
}
fn data_root() -> PathBuf {
    if let Some(v) = std::env::var_os("MORPHFORGE_DATA") {
        return v.into();
    }
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| ".".into())
                .join(".local/share")
        });
    root.join("MorphForge")
}
#[tokio::main(worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            data,
            port,
            env_file,
            no_capture,
            new_instance_if_busy,
        } => {
            serve(ServerOptions {
                root: data.unwrap_or_else(data_root),
                port,
                env_file,
                capture: !no_capture,
                isolate_if_busy: new_instance_if_busy,
            })
            .await?
        }
        Command::RecipeWorker => {
            use std::io::Read;
            let mut bytes = vec![];
            std::io::stdin()
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            let result = std::thread::Builder::new()
                .name("recipe-engine".into())
                .stack_size(16 * 1024 * 1024)
                .spawn(move || {
                    let input: recipe::WorkerInput = serde_json::from_slice(&bytes)?;
                    recipe::worker(input)
                })?
                .join()
                .map_err(|_| anyhow::anyhow!("recipe引擎线程异常"))?;
            match result {
                Ok(v) => println!("{}", v),
                Err(e) => {
                    println!("{}", serde_json::json!({"error":e.to_string()}));
                    std::process::exit(2)
                }
            }
        }
        Command::Check { file } => {
            let v = serde_json::from_slice(&std::fs::read(file)?)?;
            let scene =
                morphforge_core::Scene::import_json(&v, &morphforge_core::Limits::default())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&scene.stats(&morphforge_core::Limits::default()))?
            );
        }
        Command::Recipe { file, data, seed } => {
            let root = data.unwrap_or_else(data_root);
            let a = morphforge_app::server::App::open(root, None)?;
            let request = recipe::RecipeRequest {
                code: std::fs::read_to_string(file)?,
                seed,
                mode: "apply".into(),
                base_revision: None,
                request_id: None,
                max_ops: 100000,
                dry_run: false,
            };
            println!("{}", a.harness.run_recipe(request, None).await?);
        }
        Command::Benchmark { count } => {
            if count > 262144 {
                anyhow::bail!("基准数量超过资源保护值")
            };
            let mut ops = vec![];
            for i in 0..count {
                ops.push(morphforge_core::Operation::Box {
                    value: morphforge_core::Box3::new(
                        [
                            (i % 64) as i32 * 3 - 96,
                            (i / (64 * 64)) as i32 * 3,
                            ((i / 64) % 64) as i32 * 3 - 96,
                        ],
                        [1; 3],
                        (i % 4 + 1) as u32,
                    ),
                })
            }
            let start = std::time::Instant::now();
            let scene = morphforge_core::Scene::default()
                .apply(&ops, &morphforge_core::Limits::default())?;
            scene
                .validate(&morphforge_core::Limits::default())
                .context("基准校验")?;
            println!(
                "{}",
                serde_json::json!({"success":true,"count":scene.boxes.len(),"elapsedMs":start.elapsed().as_secs_f64()*1000.,"stats":scene.stats(&morphforge_core::Limits::default())})
            );
        }
    }
    Ok(())
}
