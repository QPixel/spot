use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use keyring::{Entry, Result};


#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub token: String,
    pub token_expiry_time: Option<SystemTime>,
    pub country: String,
}

impl Credentials {
    pub fn token_expired(&self) -> bool {
        match self.token_expiry_time {
            Some(v) => SystemTime::now() > v,
            None => true,
        }
    }

    pub async fn retrieve() -> Result<Self> {
        let service = Entry::new("spot", "default")?;
        let item = service.get_password()?;
        let creds: Self = serde_json::from_str(&item).unwrap();
        Ok(creds)
    }

    // Try to clear the credentials
    pub async fn logout() -> Result<()> {
        let service = Entry::new("spot", "default")?;
        match service.delete_password() {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("Could not delete credentials: {}", e);
                Err(e)
            },
        }
    }

    pub async fn save(&self) -> Result<()> {
        let service = Entry::new("spot", "default")?;

        // We simply write our stuct as JSON and send it
        let encoded = serde_json::to_string(&self).unwrap();
        service.set_password(&encoded)?;
        Ok(())
    }
}
