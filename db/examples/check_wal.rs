use libsql::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_path = "/tmp/test_db_crate_wal.db";
    
    // Clean up
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{}", test_path, suffix));
    }
    
    // Simulate what Database::new does
    let db = Builder::new_local(test_path).build().await?;
    let conn = db.connect()?;
    
    // Run schema (same as your init)
    let schema = include_str!("../migrations/001_schema.sql");
    conn.execute_batch(schema).await?;
    
    // Check mode
    let mut rows = conn.query("PRAGMA journal_mode", ()).await?;
    if let Some(row) = rows.next().await? {
        let mode: String = row.get(0)?;
        println!("journal_mode = {}", mode);
    }
    
    // Check files
    println!("\nFiles:");
    for entry in std::fs::read_dir("/tmp")? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("test_db_crate") {
            let meta = entry.metadata()?;
            println!("  {:?} ({} bytes)", name, meta.len());
        }
    }
    
    Ok(())
}
