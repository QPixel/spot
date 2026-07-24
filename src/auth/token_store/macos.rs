use anyhow::{Context, Result};
use keyring::Entry;

use crate::app::credentials::Credentials;

const SERVICE: &str = "dev.diegovsky.Riff";
const ACCOUNT: &str = "default";

fn entry() -> Result<Entry> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| anyhow::anyhow!("{e}"))
}

pub async fn retrieve() -> Result<Credentials> {
    let entry = entry()?;
    let password = match tokio::task::spawn_blocking(move || entry.get_password())
        .await
        .context("keychain read panicked")?
    {
        Ok(password) => password,
        Err(keyring::Error::NoEntry) => {
            return Err(anyhow::anyhow!("Empty keyring"));
        }
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    };

    if password.is_empty() {
        return Err(anyhow::anyhow!("Empty keyring"));
    }

    Ok(serde_json::from_str(&password)?)
}

pub async fn logout() -> Result<()> {
    let entry = entry()?;
    match tokio::task::spawn_blocking(move || entry.delete_password())
        .await
        .context("keychain delete panicked")?
    {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => {
            warn!("Logout attempted, but keyring is empty");
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("{e}")),
    }
}

pub async fn save(creds: &Credentials) -> Result<()> {
    info!("Saving credentials");
    let entry = entry()?;
    let json = serde_json::to_string(creds)?;
    tokio::task::spawn_blocking(move || entry.set_password(&json))
        .await
        .context("keychain write panicked")?
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    info!("Saved credentials");
    Ok(())
}
