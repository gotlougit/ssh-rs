use crate::db::{self, ProcessedKey};
use anyhow::Result;
use rusqlite::Connection;
use russh_keys::{agent::server, key::KeyPair, pkcs8};
use std::future::Future;
use std::sync::Arc;
use tokio::fs;
use tokio::net::UnixListener;

const SOCKNAME: &str = "/tmp/ssh-agent-2";

#[derive(Clone)]
struct SecureAgent {}

impl server::Agent for SecureAgent {
    fn confirm(self, _pk: Arc<KeyPair>) -> Box<dyn Future<Output = (Self, bool)> + Unpin + Send> {
        Box::new(futures::future::ready((self, true)))
    }
}

pub struct Socket {
    listener: UnixListener,
    conn: Connection,
    // FIXME: DO NOT AND I REPEAT DO NOT
    // HAVE THIS BE PLAINTEXT, EVEN IN MEMORY!
    // PROTECT THESE BITS AT ALL COSTS!
    pass: String,
    agent: SecureAgent,
}

impl Socket {
    pub fn init(pass: &str) -> Result<Self> {
        let listener = UnixListener::bind(SOCKNAME)?;
        let conn = db::open_db()?;
        // Here we would ideally place some decryption mechanisms to handle
        // sensitive key data
        Ok(Self {
            listener,
            conn,
            pass: pass.to_string(),
            agent: SecureAgent {},
        })
    }

    // TODO: make this cryptographically secure
    fn auth_req(&self, proposed_pass: &str) -> bool {
        proposed_pass == self.pass
    }

    pub fn gen_key(&self, nick: &str, user: &str, host: &str, port: u16) -> bool {
        let key = KeyPair::generate_ed25519().unwrap();
        // store this encoded key in db
        let encoded_key = pkcs8::encode_pkcs8(&key);
        crate::db::insert_key(&self.conn, nick, user, host, port, encoded_key)
    }

    pub fn show_key(&self, nick: &str) -> Result<ProcessedKey> {
        match crate::db::get_key(&self.conn, nick) {
            Ok(res) => {
                return Ok(res);
            }
            Err(e) => {
                println!("That key doesn't exist, try creating it?");
                return Err(e.into());
            }
        }
    }

    pub fn delete_key(&self, nick: &str) -> bool {
        match crate::db::del_key(&self.conn, nick) {
            Ok(rows) => rows != 0,
            Err(_) => false,
        }
    }

    pub fn show_all_keys(&self) -> Vec<ProcessedKey> {
        crate::db::get_all_keys(&self.conn).unwrap()
    }

    pub fn close(&self) {
        fs::remove_file(SOCKNAME).unwrap();
    }
}
