use morphforge_app::target::AvatarCatalog;
fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("catalog path required"))?;
    let c = AvatarCatalog::from_bytes(&std::fs::read(path)?)?;
    println!("{}", c.identity_hash());
    Ok(())
}
