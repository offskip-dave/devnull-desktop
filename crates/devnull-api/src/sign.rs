use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256, Sha512};

pub const JSON_NAMESPACE: &str = "devnull-api";
pub const MULTIPART_NAMESPACE: &str = "devnull-api-images";

/// Ed25519 key this device signs every API request with.
#[derive(Clone)]
pub struct DeviceKey {
    signing: SigningKey,
}

impl DeviceKey {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(bytes),
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// SSH wire format: string "ssh-ed25519", string 32-byte public key.
    pub fn public_key_blob(&self) -> Vec<u8> {
        ssh_blob(&self.verifying_key().to_bytes())
    }

    /// `ssh-keygen -l` fingerprint: `SHA256:<base64, no padding>`.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(self.public_key_blob());
        let b64 = STANDARD.encode(digest);
        format!("SHA256:{}", b64.trim_end_matches('='))
    }

    /// OpenSSH public key line (algorithm + base64 blob).
    pub fn openssh_public(&self) -> String {
        format!("ssh-ed25519 {}", STANDARD.encode(self.public_key_blob()))
    }

    /// Signs as `ssh-keygen -Y sign -n <namespace>` would: SSHSIG, base64 on one line.
    pub fn sign(&self, message: &[u8], namespace: &str) -> String {
        let message_hash = Sha512::digest(message);

        let mut signed_data = b"SSHSIG".to_vec();
        append_ssh_string(&mut signed_data, namespace.as_bytes());
        append_ssh_string(&mut signed_data, b"");
        append_ssh_string(&mut signed_data, b"sha512");
        append_ssh_string(&mut signed_data, &message_hash);

        let signature: Signature = self.signing.sign(&signed_data);

        let mut signature_blob = Vec::new();
        append_ssh_string(&mut signature_blob, b"ssh-ed25519");
        append_ssh_string(&mut signature_blob, &signature.to_bytes());

        let mut blob = b"SSHSIG".to_vec();
        append_u32(&mut blob, 1);
        append_ssh_string(&mut blob, &self.public_key_blob());
        append_ssh_string(&mut blob, namespace.as_bytes());
        append_ssh_string(&mut blob, b"");
        append_ssh_string(&mut blob, b"sha512");
        append_ssh_string(&mut blob, &signature_blob);

        STANDARD.encode(blob)
    }
}

fn ssh_blob(public: &[u8; 32]) -> Vec<u8> {
    let mut blob = Vec::new();
    append_ssh_string(&mut blob, b"ssh-ed25519");
    append_ssh_string(&mut blob, public);
    blob
}

fn append_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn append_ssh_string(buf: &mut Vec<u8>, value: &[u8]) {
    append_u32(buf, value.len() as u32);
    buf.extend_from_slice(value);
}

pub fn canonical_message(
    timestamp: &str,
    nonce: &str,
    method: &str,
    target: &str,
    file_hashes: Option<&str>,
    body: &[u8],
) -> Vec<u8> {
    let mut message = format!("{timestamp}\n{nonce}\n{method}\n{target}\n").into_bytes();
    if let Some(hashes) = file_hashes {
        message.extend_from_slice(hashes.as_bytes());
        message.push(b'\n');
    }
    message.extend_from_slice(body);
    message
}

pub fn random_nonce() -> String {
    let bytes: [u8; 16] = rand::random();
    hex::encode(bytes)
}
