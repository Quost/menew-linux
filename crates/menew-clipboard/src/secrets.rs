//! The Secret Service client (GNOME Keyring, KWallet) and the polkit check that guard the protected notes.
//! Lives in this crate because it already has `zbus` and `tokio`; it has nothing to do with the clipboard
//! history. Every call is blocking (a thread with its own small runtime), so call it off the UI thread when
//! it may wait for the user (a locked keyring, the polkit dialog).
//!
//! The Secret Service items carry the attributes `application=app.menew.Menew` and `note=<uuid>`, so that
//! `secret-tool lookup application app.menew.Menew note <uuid>` finds them. The secret travels over the
//! session bus with the `plain` algorithm (the bus is private to the user session, as in `libsecret` without DH).

use futures_util::StreamExt;
use menew_core::notes::{Auth, Authenticator, Vault, VaultError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use zbus::zvariant::{Dict, OwnedObjectPath, OwnedValue, Type, Value};
use zbus::{Connection, Proxy};

const SS: &str = "org.freedesktop.secrets";
const SS_PATH: &str = "/org/freedesktop/secrets";
const SS_SERVICE: &str = "org.freedesktop.Secret.Service";
const SS_COLLECTION: &str = "org.freedesktop.Secret.Collection";
const SS_ITEM: &str = "org.freedesktop.Secret.Item";
const SS_PROMPT: &str = "org.freedesktop.Secret.Prompt";
pub const APPLICATION: &str = "app.menew.Menew";
/// The polkit action of `data/app.menew.Menew.policy`.
pub const POLKIT_ACTION: &str = "app.menew.Menew.unlock-note";

/// `(oayays)`: session, parameters, value, content type.
#[derive(Serialize, Deserialize, Type, Debug)]
struct Secret {
    session: OwnedObjectPath,
    parameters: Vec<u8>,
    value: Vec<u8>,
    content_type: String,
}

/// Run a future to the end on a thread of its own, with a runtime of its own (the daemon's runtime may be
/// entered by the caller, and blocking inside it panics).
fn block_on<T: Send>(f: impl Future<Output = T> + Send) -> T {
    std::thread::scope(|s| {
        s.spawn(|| {
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(f)
        })
        .join()
        .expect("secrets thread")
    })
}

fn failed(e: impl std::fmt::Display) -> VaultError {
    VaultError::Failed(e.to_string())
}

fn is_root(p: &OwnedObjectPath) -> bool {
    p.as_str() == "/"
}

async fn run_prompt(conn: &Connection, prompt: &OwnedObjectPath) -> Result<bool, VaultError> {
    let proxy = Proxy::new(conn, SS, prompt.as_str(), SS_PROMPT).await.map_err(failed)?;
    let mut done = proxy.receive_signal("Completed").await.map_err(failed)?;
    proxy.call_method("Prompt", &("",)).await.map_err(failed)?;
    let msg = done.next().await.ok_or_else(|| failed("prompt closed"))?;
    let (dismissed, _): (bool, OwnedValue) = msg.body().deserialize().map_err(failed)?;
    Ok(!dismissed)
}

/// Unlock these objects (collection or items); `Locked` when the user dismisses the prompt.
async fn unlock(conn: &Connection, service: &Proxy<'_>, objects: Vec<OwnedObjectPath>) -> Result<(), VaultError> {
    let (_unlocked, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = service.call("Unlock", &(objects,)).await.map_err(failed)?;
    if !is_root(&prompt) && !run_prompt(conn, &prompt).await? {
        return Err(VaultError::Locked);
    }
    Ok(())
}

struct Ss<'a> {
    conn: Connection,
    service: Proxy<'a>,
    session: OwnedObjectPath,
}

async fn open() -> Result<Ss<'static>, VaultError> {
    let conn = Connection::session().await.map_err(|e| VaultError::Unavailable(e.to_string()))?;
    let service = Proxy::new(&conn, SS, SS_PATH, SS_SERVICE)
        .await
        .map_err(|e| VaultError::Unavailable(e.to_string()))?;
    let opened: Result<(OwnedValue, OwnedObjectPath), _> = service.call("OpenSession", &("plain", Value::from(""))).await;
    let (_, session) = opened.map_err(|e| VaultError::Unavailable(e.to_string()))?;
    Ok(Ss { conn, service, session })
}

impl Ss<'_> {
    fn attrs(id: &str) -> HashMap<&str, &str> {
        HashMap::from([("application", APPLICATION), ("note", id)])
    }

    /// The default collection, unlocked.
    async fn collection(&self) -> Result<OwnedObjectPath, VaultError> {
        let mut path: OwnedObjectPath = self.service.call("ReadAlias", &("default",)).await.map_err(failed)?;
        if is_root(&path) {
            // no alias yet: the "login" collection of GNOME Keyring and KWallet's compatibility layer
            path = OwnedObjectPath::try_from("/org/freedesktop/secrets/collection/login").map_err(failed)?;
        }
        let coll = Proxy::new(&self.conn, SS, path.as_str(), SS_COLLECTION).await.map_err(failed)?;
        let locked: bool = coll.get_property("Locked").await.map_err(|e| VaultError::Unavailable(e.to_string()))?;
        if locked {
            unlock(&self.conn, &self.service, vec![path.clone()]).await?;
        }
        Ok(path)
    }

    /// The item of a note, unlocked, if there is one.
    async fn find(&self, id: &str) -> Result<Option<OwnedObjectPath>, VaultError> {
        let (unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
            self.service.call("SearchItems", &(Self::attrs(id),)).await.map_err(failed)?;
        if let Some(p) = unlocked.into_iter().next() {
            return Ok(Some(p));
        }
        if let Some(p) = locked.into_iter().next() {
            unlock(&self.conn, &self.service, vec![p.clone()]).await?;
            return Ok(Some(p));
        }
        Ok(None)
    }

    fn secret(&self, text: &str) -> Secret {
        Secret { session: self.session.clone(), parameters: vec![], value: text.as_bytes().to_vec(), content_type: "text/plain; charset=utf8".into() }
    }
}

pub struct SecretServiceVault;

impl Vault for SecretServiceVault {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError> {
        block_on(async {
            let ss = open().await?;
            ss.collection().await?;
            let Some(item) = ss.find(id).await? else { return Ok(None) };
            let proxy = Proxy::new(&ss.conn, SS, item.as_str(), SS_ITEM).await.map_err(failed)?;
            let secret: Secret = proxy.call("GetSecret", &(&ss.session,)).await.map_err(failed)?;
            Ok(Some(String::from_utf8_lossy(&secret.value).into_owned()))
        })
    }

    fn write(&self, id: &str, label: &str, text: &str) -> Result<(), VaultError> {
        block_on(async {
            let ss = open().await?;
            let coll = ss.collection().await?;
            if let Some(item) = ss.find(id).await? {
                let proxy = Proxy::new(&ss.conn, SS, item.as_str(), SS_ITEM).await.map_err(failed)?;
                proxy.call_method("SetSecret", &(ss.secret(text),)).await.map_err(failed)?;
                return Ok(());
            }
            let attrs: HashMap<String, String> = HashMap::from([("application".to_string(), APPLICATION.to_string()), ("note".to_string(), id.to_string())]);
            let mut props: HashMap<&str, Value> = HashMap::new();
            props.insert("org.freedesktop.Secret.Item.Label", Value::from(label));
            props.insert("org.freedesktop.Secret.Item.Attributes", Value::from(Dict::from(attrs)));
            let proxy = Proxy::new(&ss.conn, SS, coll.as_str(), SS_COLLECTION).await.map_err(failed)?;
            let (_item, prompt): (OwnedObjectPath, OwnedObjectPath) =
                proxy.call("CreateItem", &(props, ss.secret(text), true)).await.map_err(failed)?;
            if !is_root(&prompt) && !run_prompt(&ss.conn, &prompt).await? {
                return Err(VaultError::Locked);
            }
            Ok(())
        })
    }

    fn delete(&self, id: &str) -> Result<(), VaultError> {
        block_on(async {
            let ss = open().await?;
            ss.collection().await?;
            let Some(item) = ss.find(id).await? else { return Ok(()) };
            let proxy = Proxy::new(&ss.conn, SS, item.as_str(), SS_ITEM).await.map_err(failed)?;
            let prompt: OwnedObjectPath = proxy.call("Delete", &()).await.map_err(failed)?;
            if !is_root(&prompt) {
                run_prompt(&ss.conn, &prompt).await?;
            }
            Ok(())
        })
    }
}

// ---- polkit ----

/// Start time of this process in clock ticks since boot (field 22 of `/proc/self/stat`), for the polkit subject.
fn start_time() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // the name (field 2) may hold spaces and parentheses: count from the last ')'
    stat.rsplit_once(')').map(|(_, rest)| rest).and_then(|rest| rest.split_whitespace().nth(19)).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// What `CheckAuthorization` answered, as the notes need it.
pub fn judge(authorized: bool, challenge: bool) -> Auth {
    if authorized {
        Auth::Polkit
    } else if challenge {
        Auth::KeyringOnly("no authentication agent answered".into())
    } else {
        Auth::Denied
    }
}

/// An error of polkit that means "this system cannot ask" and not "the user said no".
pub fn cannot_ask(error: &str) -> Auth {
    Auth::KeyringOnly(error.to_string())
}

pub struct PolkitAuthenticator;

impl Authenticator for PolkitAuthenticator {
    fn authenticate(&self) -> Auth {
        block_on(async {
            let conn = match Connection::system().await {
                Ok(c) => c,
                Err(e) => return cannot_ask(&format!("no system bus: {e}")),
            };
            let proxy = match Proxy::new(&conn, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority", "org.freedesktop.PolicyKit1.Authority").await {
                Ok(p) => p,
                Err(e) => return cannot_ask(&format!("no polkit: {e}")),
            };
            let mut subject: HashMap<&str, Value> = HashMap::new();
            subject.insert("pid", Value::from(std::process::id()));
            subject.insert("start-time", Value::from(start_time()));
            let details: HashMap<&str, &str> = HashMap::new();
            // flags: 1 = AllowUserInteraction
            let reply: Result<(bool, bool, HashMap<String, String>), _> =
                proxy.call("CheckAuthorization", &(("unix-process", subject), POLKIT_ACTION, details, 1u32, "")).await;
            match reply {
                Ok((authorized, challenge, _)) => judge(authorized, challenge),
                Err(e) => cannot_ask(&e.to_string()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polkit_answers_are_judged() {
        assert_eq!(judge(true, false), Auth::Polkit);
        assert_eq!(judge(false, false), Auth::Denied);
        assert!(matches!(judge(false, true), Auth::KeyringOnly(_)));
    }

    #[test]
    fn the_start_time_of_this_process_is_read() {
        if cfg!(target_os = "linux") {
            assert!(start_time() > 0);
        }
    }
}
