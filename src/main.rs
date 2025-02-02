pub use app::App;

pub mod app;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let mut args = std::env::args();
    args.next();

    let mut ws_url = "".to_string();
    if let Some(url_flag) = args.next() {
        if url_flag == "--ws" {
            if let Some(url) = args.next() {
                ws_url = url;
            }
        }
    }

    let result = App::new_with_url(ws_url).run(terminal);
    ratatui::restore();
    result
}
