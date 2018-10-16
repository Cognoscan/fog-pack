use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use super::sodium;

/// Keys are the secret data needed to act as a particular Identity.
/// - Consists of a Ed25519 private/public keypair, as well as the Curve25519 private key
/// - When encoded, it is just the "seed", from which the keypair and Curve25519 key can be derived
/// - The seed is actually the Ed25519 private key
#[derive(Clone)]
pub struct Key {
    version: u8,
    signing: sodium::SecretSignKey,
    decrypting: sodium::SecretCryptKey,
}

impl Key {

    fn blank() -> Key {
        Key {
            version: 0,
            signing: Default::default(),
            decrypting: Default::default(),
        }
    }

    fn complete(&mut self) {
        sodium::ed25519_sk_to_curve25519(&mut self.decrypting, &self.signing)
    }

    pub fn new_pair() -> Result<(Key, Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        key.version = 1;
        id.version = 1;
        sodium::sign_keypair(&mut id.signing, &mut key.signing);
        key.complete();
        id.complete()?;
        Ok((key, id))
    }

    pub fn get_identity(&self) -> Result<Identity, CryptoError> {
        let mut id = Identity::blank();
        id.version = 1;
        sodium::ed25519_sk_to_pk(&mut id.signing, &self.signing);
        id.complete()?;
        Ok(id)
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        let mut seed = Default::default();
        sodium::ed25519_sk_to_seed(&mut seed, &self.signing);
        wr.write_all(&seed.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<(Key,Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        let mut seed: sodium::Seed = Default::default();
        key.version = rd.read_u8().map_err(CryptoError::Io)?;
        id.version = key.version;
        match key.version {
            1 => {
                rd.read_exact(&mut seed.0).map_err(CryptoError::Io)?;
                sodium::sign_seed_keypair(&mut id.signing, &mut key.signing, &seed);
                key.complete();
                id.complete()?;
                Ok((key,id))
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
#[derive(Clone,PartialEq,Eq,Hash)]
pub struct Identity {
    version: u8,                         // Crypto version
    signing: sodium::PublicSignKey,       // Version 1: Ed25519 public key
    encrypting: sodium::PublicCryptKey, // Version 1: Curve 25519 public key
}

/// Consists of a Ed25519 public key and corresponding Curve25519 public key.
/// When encoded, it is just the Ed25519 public key
impl Identity {

    fn blank() -> Identity {
        Identity {
            version: 0,
            signing: Default::default(),
            encrypting: Default::default(),
        }
    }

    fn complete(&mut self) -> Result<(), CryptoError> {
        sodium::ed25519_pk_to_curve25519_pk(&mut self.encrypting, &self.signing)
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_crypt(&self) -> &sodium::PublicCryptKey {
        &self.encrypting
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.signing.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Identity, CryptoError> {
        let mut id = Identity::blank();
        id.version = rd.read_u8().map_err(CryptoError::Io)?;
        match id.version {
            1 => {
                rd.read_exact(&mut id.signing.0).map_err(CryptoError::Io)?;
                id.complete()?;
                Ok(id)
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

