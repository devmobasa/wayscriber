use anyhow::Result;

fn main() -> Result<()> {
    let schema = hyprmarker::Config::json_schema();
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
