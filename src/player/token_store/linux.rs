use anyhow::Result;
use oo7::Keyring;
use std::time::Duration;

use crate::app::credentials::Credentials;

const ATTRS: &[(&'static str, &'static str)] = &[("spot_credentials", "yes")];
const MAX_KEYRING_RETRIES: u32 = 3;
const KEYRING_RETRY_DELAY: Duration = Duration::from_millis(200);

async fn keyring() -> Keyring {
    Keyring::new().await.expect("Failed to initialize keyring")
}

pub async fn retrieve() -> Result<Credentials> {
    let keyring = keyring().await;
    if matches!(keyring, Keyring::File(_)) {
        if let Err(e) = oo7::migrate(vec![ATTRS], true).await {
            debug!("Failed to migrate system keyring: {e}");
        }
    }

    if let Err(e) = keyring.unlock().await {
        warn!("Failed to unlock keyring: {e}");
    }

    let mut last_err = None;
    for attempt in 0..MAX_KEYRING_RETRIES {
        let items = keyring.search_items(&ATTRS).await?;
        match items.first() {
            Some(item) => {
                let item_json = item.secret().await?;
                let creds = serde_json::from_slice(item_json.as_bytes())?;
                return Ok(creds);
            }
            None => {
                last_err = Some(anyhow::anyhow!("Empty keyring"));
                if attempt < MAX_KEYRING_RETRIES - 1 {
                    debug!(
                        "Keyring empty on attempt {}, retrying in {}ms...",
                        attempt + 1,
                        KEYRING_RETRY_DELAY.as_millis()
                    );
                    tokio::time::sleep(KEYRING_RETRY_DELAY).await;
                }
            }
        }
    }

    Err(last_err.unwrap())
}

pub async fn logout() -> Result<()> {
    let result = keyring().await.search_items(&ATTRS).await?;
    let Some(item) = result.first() else {
        warn!("Logout attempted, but keyring is empty");
        return Ok(());
    };
    item.delete().await?;
    Ok(())
}

pub async fn save(creds: &Credentials) -> Result<()> {
    info!("Saving credentials");
    let encoded = serde_json::to_vec(creds).unwrap();
    keyring()
        .await
        .create_item("Spotify Credentials", &ATTRS, &encoded, true)
        .await?;
    info!("Saved credentials");
    Ok(())
}
