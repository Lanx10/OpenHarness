#[cfg(not(test))]
use keyring::Entry;
use std::collections::HashMap;
use std::sync::RwLock;

#[cfg(not(test))]
const SERVICE_NAME: &str = "com.openharness.desktop";

static MEMORY_STORE: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

pub fn save_secret(account: &str, secret: &str) -> Result<(), String> {
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE_NAME, account).map_err(|e| e.to_string())?;
        entry
            .set_password(secret)
            .map_err(|e| format!("OS credential store write failed: {}", e))?;
    }
    if let Ok(mut store) = MEMORY_STORE.write() {
        store
            .get_or_insert_with(HashMap::new)
            .insert(account.to_string(), secret.to_string());
    }
    Ok(())
}

pub fn get_secret(account: &str) -> Result<Option<String>, String> {
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE_NAME, account).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(secret) => return Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(format!("OS credential store read failed: {}", e)),
        }
    }
    Ok(MEMORY_STORE
        .read()
        .map_err(|e| format!("Credential store lock failed: {}", e))?
        .as_ref()
        .and_then(|map| map.get(account).cloned()))
}

pub fn delete_secret(account: &str) -> Result<(), String> {
    if let Ok(mut store) = MEMORY_STORE.write() {
        let _ = store.as_mut().map(|map| map.remove(account));
    }
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE_NAME, account).map_err(|e| e.to_string())?;
        match entry.delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(format!("OS credential store delete failed: {}", e)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn memory_fallback_round_trip() {
        let key = format!("test-{}", TEST_ID.fetch_add(1, Ordering::Relaxed));
        if let Ok(mut store) = MEMORY_STORE.write() {
            store
                .get_or_insert_with(HashMap::new)
                .insert(key.clone(), "secret".into());
        }
        assert_eq!(
            MEMORY_STORE
                .read()
                .unwrap()
                .as_ref()
                .and_then(|map| map.get(&key).cloned()),
            Some("secret".into())
        );
    }
}
