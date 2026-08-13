use std::{env, fs, path::Path};
use xdgen::{App, Context, FluentString};

fn main() {
    let ctx = Context::new("i18n", env::var("CARGO_PKG_NAME").unwrap()).unwrap();
    let app = App::new(FluentString("cosmic-screenshot"))
        .comment(FluentString("app-comment"))
        .keywords(FluentString("app-keywords"));

    let desktop_entry = app
        .expand_desktop("data/com.system76.CosmicScreenshot.desktop", &ctx)
        .unwrap();
    let metainfo = app
        .expand_metainfo("data/com.system76.CosmicScreenshot.metainfo.xml", &ctx)
        .unwrap();

    let output = Path::new("target/xdgen/");
    fs::create_dir_all(output).unwrap();
    fs::write(
        output.join("com.system76.CosmicScreenshot.desktop"),
        desktop_entry,
    )
    .unwrap();
    fs::write(
        output.join("com.system76.CosmicScreenshot.metainfo.xml"),
        metainfo,
    )
    .unwrap();
}
