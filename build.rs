fn main() {
    slint_build::compile("ui/main.slint").unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        embed_resource::compile("app.rc", embed_resource::NONE);
    }
}
