pub mod app;

use app::App;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    url: Option<String>,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let url = args.url.unwrap_or_else(|| "".to_string());

    let terminal = ratatui::init();
    let result = App::new(url).run(terminal).await;

    ratatui::restore();
    result
}
