//! Key generation and signatures.

use std::fmt::{Debug, Formatter};

use ed25519_dalek::{Keypair, SecretKey, Signer};
use getrandom::getrandom;
use hkdf::Hkdf;
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha20Rng,
};
use sha2::Sha256;

pub type RootSecret = [u8; 32];
pub type PubkeyBytes = [u8; 32];
pub type SymmetricKey = [u8; 32];
pub type Signature = [u8; 64];

/// A struct that handles key generation, signatures and key derivation.
pub struct KeyManager {
    root_secret: RootSecret,
    signature_keypair: Keypair,
    backup_secret_key: SymmetricKey,
}

impl Debug for KeyManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyManager")
    }
}

impl KeyManager {
    /// Generate a new key manager with a new root secret.
    pub fn generate() -> anyhow::Result<Self> {
        let mut root_secret: RootSecret = Default::default();
        getrandom(&mut root_secret)?;

        Self::from_secret(root_secret)
    }

    /// Initialize a key manager from an existing root secret.
    pub fn from_secret(root_secret: RootSecret) -> anyhow::Result<Self> {
        // seed our CSPRNG with the root secret to generate keys reproducibly
        let mut csprng = ChaCha20Rng::from_seed(root_secret);

        // take 32 bytes of CSPRNG's output to generate the Ed25519 keypair
        let mut privkey: [u8; 32] = Default::default();
        csprng.fill_bytes(&mut privkey);

        // I would normally use the Keypair::generate function, but the versions
        // of the rand crate between dalek and chacha don't match so it doesn't compile.
        // This is the same method of generating the key used by the original library.
        let privkey = SecretKey::from_bytes(&privkey)?;
        let signature_keypair = Keypair { public: (&privkey).into(), secret: privkey };

        // take another 32 bytes of CSPRNG's output for the symmetric secret
        let mut backup_secret_key: SymmetricKey = Default::default();
        csprng.fill_bytes(&mut backup_secret_key);

        Ok(Self {
            root_secret,
            signature_keypair,
            backup_secret_key,
        })
    }

    /// Get the root secret of the key manager.
    pub fn get_root_secret(&self) -> RootSecret {
        // it would be nicer to not have this, but the CLI currently needs it
        self.root_secret
    }

    /// Get the public key of the signature keypair.
    pub fn get_pubkey(&self) -> PubkeyBytes {
        self.signature_keypair.public.to_bytes()
    }

    /// Sign a message with the signature keypair.
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signature_keypair.sign(data).to_bytes()
    }

    /// Derive a symmetric key from the backup secret key with a KDF.
    pub fn derive_backup_key(&self, info: &[u8]) -> SymmetricKey {
        let kdf = Hkdf::<Sha256>::from_prk(&self.backup_secret_key).unwrap();
        let mut output: SymmetricKey = Default::default();
        kdf.expand(info, &mut output).unwrap();

        output
    }
}
