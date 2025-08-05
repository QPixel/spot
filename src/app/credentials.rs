use keyring::{Entry, Error, Result};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub token_expiry_time: Option<SystemTime>,
}

impl Credentials {
    pub fn token_expired(&self) -> bool {
        match self.token_expiry_time {
            Some(v) => SystemTime::now() > v,
            None => true,
        }
    }

    // pub async fn retrieve() -> Result<Self, Error> {
    //     let service = SecretService::connect(EncryptionType::Dh).await?;
    //     let collection = service.get_default_collection().await?;
    //     if collection.is_locked().await? {
    //         collection.unlock().await?;
    //     }
    //     let items = collection.search_items(make_attributes()).await?;
    //     let item = items.first().ok_or(Error::NoResult)?.get_secret().await?;
    //     serde_json::from_slice(&item).map_err(|_| Error::Unavailable)
    // }
    pub async fn retrieve() -> Result<Self> {
        let service = Entry::new("spot", "default")?;
        if let Ok(password) = service.get_password() {
            if password.is_empty() {
                Ok(Self {
                    access_token: String::new(),
                    refresh_token: String::new(),
                    token_expiry_time: None,
                })
            } else {
                let creds = serde_json::from_str(&password).unwrap();
                Ok(creds)
            }
        } else {
            Ok(Self {
                access_token: String::new(),
                refresh_token: String::new(),
                token_expiry_time: None,
            })
        }
    }

    // Try to clear the credentials
    // pub async fn logout() -> Result<(), Error> {
    //     let service = SecretService::connect(EncryptionType::Dh).await?;
    //     let collection = service.get_default_collection().await?;
    //     if !collection.is_locked().await? {
    //         let result = collection.search_items(make_attributes()).await?;
    //         let item = result.first().ok_or(Error::NoResult)?;
    //         item.delete().await
    //     } else {
    //         warn!("Keyring is locked -- not clearing credentials");
    //         Ok(())
    //     }
    // }

    pub async fn logout() -> Result<()> {
        let service = Entry::new("spot", "default")?;
        service.delete_password();
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        let service = Entry::new("spot", "default")?;

        // We simply write our stuct as JSON and send it
        info!("Saving credentials");
        let encoded = serde_json::to_string(self).unwrap();
        service.set_password(&encoded).unwrap();
        info!("Saved credentials");
        Ok(())
    }
}
