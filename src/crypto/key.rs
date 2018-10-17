use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::{CryptoError, Hash};
use super::sodium::*;

#[derive(Clone)]
pub struct Signature {
    id_version: u8,
    id: PublicSignKey,
    hash_version: u8,
    sig: Sign,
}

impl Signature {
    pub fn get_hash_version(&self) -> u8 {
        self.hash_version
    }

    pub fn get_identity_version(&self) -> u8 {
        self.id_version
    }

    pub fn signed_by(&self) -> &PublicSignKey {
        &self.id
    }

    pub fn verify(&self, hash: &Hash) -> bool {
        verify_detached(&self.id, hash.as_bytes(), &self.sig)
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.id_version)?;
        wr.write_u8(self.hash_version)?;
        wr.write_all(&self.id.0)?;
        wr.write_all(&self.sig.0)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Signature, CryptoError> {
        let id_version = rd.read_u8().map_err(CryptoError::Io)?;
        let hash_version = rd.read_u8().map_err(CryptoError::Io)?;
        if id_version != 1 || hash_version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut id: PublicSignKey = Default::default();
        let mut sig: Sign = Default::default();
        rd.read_exact(&mut id.0).map_err(CryptoError::Io)?;
        rd.read_exact(&mut sig.0).map_err(CryptoError::Io)?;
        Ok(Signature {
            id_version,
            id,
            hash_version,
            sig
        })
    }
}

/// Keys are the secret data needed to act as a particular Identity.
/// - Consists of a Ed25519 private/public keypair, as well as the Curve25519 private key
/// - When encoded, it is just the "seed", from which the keypair and Curve25519 key can be derived
/// - The seed is actually the Ed25519 private key
#[derive(Clone)]
pub struct Key {
    version: u8,
    signing: SecretSignKey,
    decrypting: SecretCryptKey,
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
        ed25519_sk_to_curve25519(&mut self.decrypting, &self.signing)
    }

    pub fn new_pair() -> Result<(Key, Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        key.version = 1;
        id.version = 1;
        sign_keypair(&mut id.signing, &mut key.signing);
        key.complete();
        id.complete()?;
        Ok((key, id))
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_id(&self) -> PublicSignKey {
        let mut pk: PublicSignKey = Default::default();
        ed25519_sk_to_pk(&mut pk, &self.signing);
        pk
    }

    pub fn get_identity(&self) -> Result<Identity, CryptoError> {
        let mut id = Identity::blank();
        id.version = 1;
        ed25519_sk_to_pk(&mut id.signing, &self.signing);
        id.complete()?;
        Ok(id)
    }

    pub fn calc_stream_key(&self, pk: &PublicCryptKey) -> Result<SecretKey, CryptoError> {
        calc_secret(pk, &self.decrypting)
    }

    pub fn sign(&self, hash: &Hash) -> Signature {
        Signature { 
            id_version: self.version,
            hash_version: hash.get_version(),
            id: self.get_id(),
            sig: sign_detached(&self.signing, hash.as_bytes())
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        let mut seed = Default::default();
        ed25519_sk_to_seed(&mut seed, &self.signing);
        wr.write_all(&seed.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<(Key,Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        let mut seed: Seed = Default::default();
        key.version = rd.read_u8().map_err(CryptoError::Io)?;
        id.version = key.version;
        match key.version {
            1 => {
                rd.read_exact(&mut seed.0).map_err(CryptoError::Io)?;
                sign_seed_keypair(&mut id.signing, &mut key.signing, &seed);
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
    signing: PublicSignKey,       // Version 1: Ed25519 public key
    encrypting: PublicCryptKey, // Version 1: Curve 25519 public key
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
        ed25519_pk_to_curve25519_pk(&mut self.encrypting, &self.signing)
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_id(&self) -> PublicSignKey {
        self.signing.clone()
    }

    pub fn calc_stream_key(&self, sk: &SecretCryptKey) -> Result<SecretKey, CryptoError> {
        calc_secret(&self.encrypting, sk)
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

