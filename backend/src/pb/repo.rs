use crate::config::AppConfig;
use crate::error::AppResult;
use crate::models::{
    account::{Account, AccountConfigRecord, EncryptedConfigRecord},
    user::{User, UserConfigRecord},
};
use crate::pb::{pb_escape, PocketBaseClient};
use serde_json::{json, Value};

impl PocketBaseClient {
    /// Returns the primary account (or the specified account by id).
    pub async fn resolve_account(&self, account_id: Option<&str>) -> AppResult<Option<Account>> {
        if let Some(id) = account_id {
            if !id.is_empty() {
                if let Ok(acc) = self.get_by_id::<Account>("accounts", id).await {
                    return Ok(Some(acc));
                }
                let filter = format!("uuid = '{}'", pb_escape(id));
                if let Some(acc) = self.find_first::<Account>("accounts", &filter, None).await? {
                    return Ok(Some(acc));
                }
            }
        }
        self.find_first::<Account>("accounts", "archived_at = ''", Some("created"))
            .await
    }

    /// Reads an `account_configs` JSON value for `(account_id, key)`, with single-tenant fallback
    /// to the primary account when `account_id` is None.
    pub async fn get_account_config(
        &self,
        account_id: Option<&str>,
        key: &str,
    ) -> AppResult<Option<Value>> {
        if let Some(acc_id) = account_id {
            if !acc_id.is_empty() {
                let filter = format!(
                    "account_id = '{}' && key = '{}'",
                    pb_escape(acc_id),
                    pb_escape(key)
                );
                if let Some(rec) = self
                    .find_first::<AccountConfigRecord>("account_configs", &filter, None)
                    .await?
                {
                    return Ok(Some(rec.value));
                }
            }
        }

        if let Some(primary) = self.resolve_account(None).await? {
            let filter = format!(
                "account_id = '{}' && key = '{}'",
                pb_escape(&primary.id),
                pb_escape(key)
            );
            if let Some(rec) = self
                .find_first::<AccountConfigRecord>("account_configs", &filter, None)
                .await?
            {
                return Ok(Some(rec.value));
            }
        }

        Ok(None)
    }

    /// Upserts an `account_configs` JSON value for `(account_id, key)`.
    pub async fn set_account_config(
        &self,
        account_id: &str,
        key: &str,
        value: Value,
    ) -> AppResult<AccountConfigRecord> {
        let filter = format!(
            "account_id = '{}' && key = '{}'",
            pb_escape(account_id),
            pb_escape(key)
        );
        if let Some(existing) = self
            .find_first::<AccountConfigRecord>("account_configs", &filter, None)
            .await?
        {
            self.update(
                "account_configs",
                &existing.id,
                &json!({
                    "value": value
                }),
            )
            .await
        } else {
            self.create(
                "account_configs",
                &json!({
                    "account_id": account_id,
                    "key": key,
                    "value": value
                }),
            )
            .await
        }
    }

    /// Reads and decrypts an `encrypted_configs` JSON payload for `(account_id, key)`.
    pub async fn get_encrypted_config(
        &self,
        app_config: &AppConfig,
        account_id: Option<&str>,
        key: &str,
    ) -> AppResult<Option<Value>> {
        if let Some(acc_id) = account_id {
            if !acc_id.is_empty() {
                let filter = format!(
                    "account_id = '{}' && key = '{}'",
                    pb_escape(acc_id),
                    pb_escape(key)
                );
                if let Some(rec) = self
                    .find_first::<EncryptedConfigRecord>("encrypted_configs", &filter, None)
                    .await?
                {
                    if let Ok(val) = app_config.decrypt_json(&rec.value_encrypted) {
                        return Ok(Some(val));
                    }
                }
            }
        }

        if let Some(primary) = self.resolve_account(None).await? {
            let filter = format!(
                "account_id = '{}' && key = '{}'",
                pb_escape(&primary.id),
                pb_escape(key)
            );
            if let Some(rec) = self
                .find_first::<EncryptedConfigRecord>("encrypted_configs", &filter, None)
                .await?
            {
                if let Ok(val) = app_config.decrypt_json(&rec.value_encrypted) {
                    return Ok(Some(val));
                }
            }
        }

        Ok(None)
    }

    /// Encrypts and upserts an `encrypted_configs` JSON payload for `(account_id, key)`.
    pub async fn set_encrypted_config(
        &self,
        app_config: &AppConfig,
        account_id: &str,
        key: &str,
        value: &Value,
    ) -> AppResult<EncryptedConfigRecord> {
        let encrypted = app_config
            .encrypt_json(value)
            .map_err(crate::error::AppError::Internal)?;

        let filter = format!(
            "account_id = '{}' && key = '{}'",
            pb_escape(account_id),
            pb_escape(key)
        );
        if let Some(existing) = self
            .find_first::<EncryptedConfigRecord>("encrypted_configs", &filter, None)
            .await?
        {
            self.update(
                "encrypted_configs",
                &existing.id,
                &json!({
                    "value_encrypted": encrypted
                }),
            )
            .await
        } else {
            self.create(
                "encrypted_configs",
                &json!({
                    "account_id": account_id,
                    "key": key,
                    "value_encrypted": encrypted
                }),
            )
            .await
        }
    }

    /// Deletes an `encrypted_configs` record for `(account_id, key)`.
    pub async fn delete_encrypted_config(&self, account_id: &str, key: &str) -> AppResult<()> {
        let filter = format!(
            "account_id = '{}' && key = '{}'",
            pb_escape(account_id),
            pb_escape(key)
        );
        if let Some(existing) = self
            .find_first::<EncryptedConfigRecord>("encrypted_configs", &filter, None)
            .await?
        {
            self.delete("encrypted_configs", &existing.id).await?;
        }
        Ok(())
    }

    /// Reads a `user_configs` JSON value for `(user_id, key)`.
    pub async fn get_user_config(&self, user_id: &str, key: &str) -> AppResult<Option<Value>> {
        let filter = format!(
            "user_id = '{}' && key = '{}'",
            pb_escape(user_id),
            pb_escape(key)
        );
        if let Some(rec) = self
            .find_first::<UserConfigRecord>("user_configs", &filter, None)
            .await?
        {
            return Ok(Some(rec.value));
        }
        Ok(None)
    }

    /// Upserts a `user_configs` JSON value for `(user_id, key)`.
    pub async fn set_user_config(
        &self,
        user_id: &str,
        key: &str,
        value: Value,
    ) -> AppResult<UserConfigRecord> {
        let filter = format!(
            "user_id = '{}' && key = '{}'",
            pb_escape(user_id),
            pb_escape(key)
        );
        if let Some(existing) = self
            .find_first::<UserConfigRecord>("user_configs", &filter, None)
            .await?
        {
            self.update("user_configs", &existing.id, &json!({ "value": value }))
                .await
        } else {
            self.create(
                "user_configs",
                &json!({
                    "user_id": user_id,
                    "key": key,
                    "value": value
                }),
            )
            .await
        }
    }

    /// Checks whether initial application setup has been completed (at least 1 active user exists).
    pub async fn has_any_user(&self) -> AppResult<bool> {
        let res = self
            .list::<User>("app_users", 1, 1, Some("archived_at = ''"), None)
            .await?;
        Ok(res.total_items > 0)
    }
}
