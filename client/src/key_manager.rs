use ed25519_dalek::{Keypair, SecretKey, Signer};
use getrandom::getrandom;
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha20Rng,
};

type Secret = [u8; 32];
type PubkeyBytes = [u8; 32];
type SymmetricKey = [u8; 32];
type Signature = [u8; 64];

struct KeyManager {
    master_secret: Secret,
    signature_keypair: Keypair,
    backup_secret_key: SymmetricKey,
}

impl KeyManager {
    pub fn generate() -> anyhow::Result<Self> {
        let mut master_secret: Secret = Default::default();
        getrandom(&mut master_secret)?;

        Self::from_secret(master_secret)
    }

    pub fn from_secret(master_secret: Secret) -> anyhow::Result<Self> {
        // seed our CSPRNG with the master secret to generate keys reproducibly
        let mut csprng = ChaCha20Rng::from_seed(master_secret);

        // take 32 bytes of CSPRNG's output to generate the Ed25519 keypair
        let mut privkey: [u8; 32] = Default::default();
        csprng.fill_bytes(&mut privkey);

        // I would normally use the Keypair::generate function, but the versions
        // of the rand crate between dalek and chacha don't match so it doesn't compile.
        // This is the same method of generating the key used by the original library.
        let privkey = SecretKey::from_bytes(&privkey)?;
        let signature_keypair = Keypair {
            public: (&privkey).into(),
            secret: privkey,
        };

        // take another 32 bytes of CSPRNG's output for the symmetric secret
        let mut backup_secret_key: SymmetricKey = Default::default();
        csprng.fill_bytes(&mut backup_secret_key);

        Ok(Self {
            master_secret,
            signature_keypair,
            backup_secret_key,
        })
    }

    pub fn get_pubkey(&self) -> PubkeyBytes {
        self.signature_keypair.public.to_bytes()
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signature_keypair.sign(data).to_bytes()
    }
}
