pub mod app;
pub mod dark_mode;

use anyhow::Result;

pub async fn run_app() -> Result<()> {
    app::run().await
}
