use libsql::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    
    // Test each pragma individually
    let pragmas = [
        ("journal_mode", "PRAGMA journal_mode"),
        ("synchronous", "PRAGMA synchronous"),
        ("cache_size", "PRAGMA cache_size"),
        ("temp_store", "PRAGMA temp_store"),
        ("mmap_size", "PRAGMA mmap_size"),
        ("page_size", "PRAGMA page_size"),
    ];
    
    println!("=== libSQL Default PRAGMA Values ===\n");
    
    for (name, query) in pragmas {
        match conn.query(query, ()).await {
            Ok(mut rows) => {
                if let Some(row) = rows.next().await? {
                    let val: String = row.get(0)?;
                    println!("{:15} = {}", name, val);
                }
            }
            Err(e) => println!("{:15} = ERROR: {}", name, e),
        }
    }
    
    println!("\n=== Testing PRAGMA Modifications ===\n");
    
    // Try setting WAL mode
    match conn.execute("PRAGMA journal_mode = WAL", ()).await {
        Ok(_) => {
            let mut rows = conn.query("PRAGMA journal_mode", ()).await?;
            if let Some(row) = rows.next().await? {
                let val: String = row.get(0)?;
                println!("Set journal_mode = WAL: now = {}", val);
            }
        }
        Err(e) => println!("Failed to set journal_mode: {}", e),
    }
    
    // Try setting cache_size
    match conn.execute("PRAGMA cache_size = -64000", ()).await {
        Ok(_) => {
            let mut rows = conn.query("PRAGMA cache_size", ()).await?;
            if let Some(row) = rows.next().await? {
                let val: i64 = row.get(0)?;
                println!("Set cache_size = -64000: now = {}", val);
            }
        }
        Err(e) => println!("Failed to set cache_size: {}", e),
    }
    
    // Try setting synchronous
    match conn.execute("PRAGMA synchronous = NORMAL", ()).await {
        Ok(_) => {
            let mut rows = conn.query("PRAGMA synchronous", ()).await?;
            if let Some(row) = rows.next().await? {
                let val: i64 = row.get(0)?;
                println!("Set synchronous = NORMAL: now = {} (1=NORMAL, 2=FULL)", val);
            }
        }
        Err(e) => println!("Failed to set synchronous: {}", e),
    }
    
    Ok(())
}
