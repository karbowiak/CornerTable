fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,gpui::hot_reload=info"),
    )
    .target(env_logger::Target::Stdout)
    .init();
    corner_table::run()
}
