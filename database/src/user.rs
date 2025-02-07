use crate::DatabaseError;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

use ring::digest;
use ring::pbkdf2;

static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;
const CREDENTIAL_LEN: usize = digest::SHA256_OUTPUT_LEN;
const HASH_ROUNDS: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(1_000) };

pub type Credential = [u8; CREDENTIAL_LEN];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    Black,
}

pub fn default_theme() -> Theme {
    Theme::Dark
}

pub fn default_true() -> bool {
    true
}

pub fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultVideoQuality {
    /// Represents DirectPlay quality
    DirectPlay,
    /// Represents a default video quality made up of resolution and bitrate.
    Resolution(u64, u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// Theme of the app
    #[serde(default = "default_theme")]
    theme: Theme,
    /// Defines whether the sidebar should be collapsed or not
    #[serde(default = "default_false")]
    is_sidebar_compact: bool,
    #[serde(default = "default_true")]
    show_card_names: bool,
    /// If this contains a string then the filebrowser/explorer will default to this path instead of `/`.
    filebrowser_default_path: Option<String>,
    #[serde(default = "default_true")]
    filebrowser_list_view: bool,
    /// If a file has subtitles then the subtitles with this language will be selected.
    default_subtitle_language: Option<String>,
    /// If a file has audio then the audio track with this language will be selected, otherwise the first one.
    default_audio_language: Option<String>,
    /// Represents the default video quality for user.
    pub default_video_quality: DefaultVideoQuality,
    /// Any other external args.
    #[serde(default)]
    external_args: HashMap<String, String>,
    /// Whether hovercards are hidden or not
    #[serde(default)]
    show_hovercards: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            is_sidebar_compact: false,
            show_card_names: true,
            filebrowser_default_path: None,
            filebrowser_list_view: true,
            default_subtitle_language: Some("english".into()),
            default_audio_language: Some("english".into()),
            external_args: HashMap::new(),
            show_hovercards: true,
            default_video_quality: DefaultVideoQuality::DirectPlay,
        }
    }
}

// NOTE: Figure out the bug with this not being a valid postgres type
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    User,
}

#[derive(Debug)]
pub struct User {
    pub username: String,
    pub roles: Vec<String>,
    pub password: String,
    pub prefs: UserSettings,
    pub picture: Option<i64>,
}

impl User {
    /// Method gets all entries from the table users.
    ///
    /// # Arguments
    ///
    /// * `&` - postgres &ection
    pub async fn get_all(conn: &mut crate::Transaction<'_>) -> Result<Vec<Self>, DatabaseError> {
        Ok(sqlx::query!("SELECT * FROM users")
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|user| Self {
                username: user.username.unwrap(),
                roles: user.roles.split(',').map(ToString::to_string).collect(),
                password: user.password,
                prefs: serde_json::from_slice(&user.prefs).unwrap_or_default(),
                picture: user.picture,
            })
            .collect())
    }

    pub async fn get(
        conn: &mut crate::Transaction<'_>,
        username: &str,
    ) -> Result<Self, DatabaseError> {
        Ok(sqlx::query!(
            "SELECT * from users
                WHERE username = ?",
            username
        )
        .fetch_one(&mut *conn)
        .await
        .map(|u| Self {
            username: u.username.unwrap(),
            roles: u.roles.split(',').map(ToString::to_string).collect(),
            password: u.password,
            prefs: serde_json::from_slice(&u.prefs).unwrap_or_default(),
            picture: u.picture,
        })?)
    }

    /// Method gets one entry from the table users based on the username supplied and password.
    ///
    /// # Arguments
    /// * `&` - postgres &ection
    /// * `uname` - username we wish to target and delete
    /// * `pw_hash` - hash of the password for the user we are trying to access
    pub async fn get_one(
        conn: &mut crate::Transaction<'_>,
        uname: String,
        pw: String,
    ) -> Result<Self, DatabaseError> {
        let hash = hash(uname.clone(), pw);
        let user = sqlx::query!(
            "SELECT * FROM users WHERE username = ? AND password = ?",
            uname,
            hash,
        )
        .fetch_one(&mut *conn)
        .await?;

        Ok(Self {
            username: user.username.unwrap(),
            roles: user.roles.split(',').map(ToString::to_string).collect(),
            password: user.password,
            prefs: serde_json::from_slice(&user.prefs).unwrap_or_default(),
            picture: user.picture,
        })
    }

    /// Method deletes a entry from the table users and returns the number of rows deleted.
    /// NOTE: Return should always be 1
    ///
    /// # Arguments
    /// * `&` - postgres &ection
    /// * `uname` - username we wish to target and delete
    pub async fn delete(
        conn: &mut crate::Transaction<'_>,
        uname: String,
    ) -> Result<usize, DatabaseError> {
        Ok(sqlx::query!("DELETE FROM users WHERE username = ?", uname)
            .execute(&mut *conn)
            .await?
            .rows_affected() as usize)
    }

    /// Method resets the password for a user to a new password.
    ///
    /// # Arguments
    /// * `&` - db &ection
    /// * `password` - new password.
    pub async fn set_password(
        &self,
        conn: &mut crate::Transaction<'_>,
        password: String,
    ) -> Result<usize, DatabaseError> {
        let hash = hash(self.username.clone(), password);

        Ok(sqlx::query!(
            "UPDATE users SET password = $1 WHERE username = ?2",
            hash,
            self.username
        )
        .execute(&mut *conn)
        .await?
        .rows_affected() as usize)
    }

    pub async fn set_username(
        conn: &mut crate::Transaction<'_>,
        old_username: String,
        new_username: String,
    ) -> Result<usize, DatabaseError> {
        Ok(sqlx::query!(
            "UPDATE users SET username = $1 WHERE users.username = ?2",
            new_username,
            old_username
        )
        .execute(&mut *conn)
        .await?
        .rows_affected() as usize)
    }

    pub async fn set_picture(
        conn: &mut crate::Transaction<'_>,
        username: String,
        asset_id: i64,
    ) -> Result<usize, DatabaseError> {
        Ok(sqlx::query!(
            "UPDATE users SET picture = $1 WHERE users.username = ?2",
            asset_id,
            username
        )
        .execute(&mut *conn)
        .await?
        .rows_affected() as usize)
    }
}

#[derive(Deserialize)]
pub struct InsertableUser {
    pub username: String,
    pub password: String,
    pub roles: Vec<String>,
    pub prefs: UserSettings,
    pub claimed_invite: String,
}

impl InsertableUser {
    /// Method consumes a InsertableUser object and inserts the values under it into postgres users
    /// table as a new user
    ///
    /// # Arguments
    /// * `self` - instance of InsertableUser which gets consumed
    /// * `&` - postgres &ection
    pub async fn insert(self, conn: &mut crate::Transaction<'_>) -> Result<String, DatabaseError> {
        let Self {
            username,
            password,
            roles,
            prefs,
            claimed_invite,
        } = self;

        let password = hash(username.clone(), password);
        let roles = roles.join(",");
        let prefs = serde_json::to_vec(&prefs).unwrap_or_default();

        sqlx::query!(
            "INSERT INTO users (username, password, prefs, claimed_invite, roles) VALUES ($1, $2, $3, $4, $5)",
            username,
            password,
            prefs,
            claimed_invite,
            roles
        )
        .execute(&mut *conn)
        .await?;

        Ok(username)
    }
}

#[derive(Deserialize)]
pub struct UpdateableUser {
    pub prefs: Option<UserSettings>,
}

impl UpdateableUser {
    pub async fn update(
        &self,
        conn: &mut crate::Transaction<'_>,
        user: &str,
    ) -> Result<usize, DatabaseError> {
        if let Some(prefs) = &self.prefs {
            let prefs = serde_json::to_vec(&prefs).unwrap_or_default();
            return Ok(sqlx::query!(
                "UPDATE users SET prefs = $1 WHERE users.username = ?2",
                prefs,
                user
            )
            .execute(&mut *conn)
            .await?
            .rows_affected() as usize);
        }

        Ok(0)
    }
}

#[derive(Deserialize, Default)]
pub struct Login {
    pub username: String,
    pub password: String,
    pub invite_token: Option<String>,
}

impl Login {
    /// Will return whether the token is valid and hasnt been claimed yet.
    pub async fn invite_token_valid(
        &self,
        conn: &mut crate::Transaction<'_>,
    ) -> Result<bool, DatabaseError> {
        let tok = match &self.invite_token {
            None => return Ok(false),
            Some(t) => t,
        };

        Ok(sqlx::query!(
            "SELECT id FROM invites
                          WHERE id NOT IN (
                              SELECT claimed_invite FROM users
                          )
                          AND id = ?",
            tok
        )
        .fetch_optional(&mut *conn)
        .await?
        .is_some())
    }

    pub async fn invalidate_token(
        &self,
        conn: &mut crate::Transaction<'_>,
    ) -> Result<usize, DatabaseError> {
        if let Some(tok) = &self.invite_token {
            Ok(sqlx::query!("DELETE FROM invites WHERE id = ?", tok)
                .execute(&mut *conn)
                .await?
                .rows_affected() as usize)
        } else {
            Ok(0)
        }
    }

    pub async fn new_invite(conn: &mut crate::Transaction<'_>) -> Result<String, DatabaseError> {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let token = uuid::Uuid::new_v4().to_hyphenated().to_string();
        let _ = sqlx::query!(
            "INSERT INTO invites (id, date_added) VALUES ($1, $2)",
            token,
            ts
        )
        .execute(&mut *conn)
        .await?;

        Ok(token)
    }

    pub async fn get_all_invites(
        conn: &mut crate::Transaction<'_>,
    ) -> Result<Vec<String>, DatabaseError> {
        Ok(sqlx::query!("SELECT id from invites")
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect())
    }

    pub async fn delete_token(
        conn: &mut crate::Transaction<'_>,
        token: String,
    ) -> Result<usize, DatabaseError> {
        Ok(sqlx::query!(
            "DELETE FROM invites
                WHERE id NOT IN (
                    SELECT claimed_invite FROM users
                ) AND id = ?",
            token
        )
        .execute(&mut *conn)
        .await?
        .rows_affected() as usize)
    }
}

pub fn hash(salt: String, s: String) -> String {
    let mut to_store: Credential = [0u8; CREDENTIAL_LEN];
    pbkdf2::derive(
        PBKDF2_ALG,
        HASH_ROUNDS,
        &salt.as_bytes(),
        s.as_bytes(),
        &mut to_store,
    );
    base64::encode(&to_store)
}

pub fn verify(salt: String, password: String, attempted_password: String) -> bool {
    let real_pwd = base64::decode(&password).unwrap();

    pbkdf2::verify(
        PBKDF2_ALG,
        HASH_ROUNDS,
        &salt.as_bytes(),
        attempted_password.as_bytes(),
        real_pwd.as_slice(),
    )
    .is_ok()
}
