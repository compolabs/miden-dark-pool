use serde::{Deserialize, Serialize};

// the payload vector is the serialized note
// id is the noteId
#[derive(Serialize, Deserialize, Debug)]
pub struct MidenNote {
    pub id: String,
    pub payload: Vec<u8>,
}

pub async fn delete_keystore_and_store(user_id: &str) {
    // Remove the SQLite store file

    let keystore_dir: &str = &format!("./keystore_{}", user_id);
    let store_path: &str = &format!("./store_{}.sqlite3", user_id);

    if tokio::fs::metadata(store_path).await.is_ok() {
        if let Err(e) = tokio::fs::remove_file(store_path).await {
            eprintln!("failed to remove {}: {}", store_path, e);
        }
    } else {
        println!("store not found: {}", store_path);
    }

    // Remove all files in the ./keystore directory
    match tokio::fs::read_dir(keystore_dir).await {
        Ok(mut dir) => {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let file_path = entry.path();
                if let Err(e) = tokio::fs::remove_file(&file_path).await {
                    eprintln!("failed to remove {}: {}", file_path.display(), e);
                }
            }
        }
        Err(e) => eprintln!("failed to read directory {}: {}", keystore_dir, e),
    }
}
