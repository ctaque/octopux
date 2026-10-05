use async_trait::async_trait;
use email_address::EmailAddress;
use octopux::BeforeSave;
use anyhow::Result;
use argon2::{password_hash::PasswordHasher, Argon2};

use crate::my_model::{NewMyModel, UpdatableMyModel};
use crate::my_child_model::{NewMyChildModel, UpdatableMyChildModel};
use crate::shared::AppState;

// Hashes the password before the insert, so it is never stored in clear
#[async_trait]
impl BeforeSave<AppState> for NewMyModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        self.password = Argon2::default().hash_password(self.password.as_bytes())?.to_string();
        Ok(self)
    }
}

// Called before the update
#[async_trait]
impl BeforeSave<AppState> for UpdatableMyModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        println!("before save hook (update)");
        Ok(self)
    }
}


#[async_trait]
impl BeforeSave<AppState> for NewMyChildModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        if EmailAddress::is_valid(&self.email) == false {
            anyhow::bail!("Invalid Email");
        }
        Ok(self)
    }
}

#[async_trait]
impl BeforeSave<AppState> for UpdatableMyChildModel {
    async fn before_save(mut self: Self, _: &AppState) -> Result<Self> {
        if EmailAddress::is_valid(&self.email) == false {
            anyhow::bail!("Invalid Email");
        }
        Ok(self)
    }
}
