use async_trait::async_trait;
use octopux::BeforeSave;
use anyhow::Result;
use argon2::{password_hash::PasswordHasher, Argon2};

use crate::model::{NewModel, UpdatableModel};
use crate::shared::AppState;

// Hashes the password before the insert, so it is never stored in clear
#[async_trait]
impl BeforeSave<AppState> for NewModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        self.password = Argon2::default().hash_password(self.password.as_bytes())?.to_string();
        Ok(self)
    }
}

// Called before the update
#[async_trait]
impl BeforeSave<AppState> for UpdatableModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        println!("before save hook (update)");
        Ok(self)
    }
}
