use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::{CryptoError, Hash};
use super::sodium::*;

#[derive(Clone,PartialEq,Eq,Hash)]
pub struct Key {
    version: u8,
    id: PublicSignKey,
}
#[derive(Clone,PartialEq,Eq,Hash)]
pub struct Identity {
    version: u8,
    id: PublicSignKey,
}

impl Key {
    pub fn get_identity(&self) -> Identity {
        Identity { version: self.version, id: self.id.clone() }
    }
}

impl Identity {
    pub fn get_key(&self) -> Key {
        Key { version: self.version, id: self.id.clone() }
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ ver={}, {:x?} }}", stringify!(Key), &self.version, &self.id.0[..])
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ ver={}, {:x?} }}", stringify!(Identity), &self.version, &self.id.0[..])
    }
}

pub fn key_from_id(version: u8, id: PublicSignKey) -> Key {
    Key { version, id }
}

pub fn identity_from_id(version: u8, id: PublicSignKey) -> Identity {
    Identity { version, id }
}

/// FullSignatures are used to authenticate a piece of data based on its hash. One can be generated 
/// from a [`FullKey`] and a [`Hash`]. The versions of each are stored, along with the identifying 
/// information of the FullKey used (i.e. the public signing key).
///
/// It can be verified by providing the hash of the data. The signing identity can be determined 
/// using the public signing key retrieved by `get_identity_version`.
#[derive(Debug,PartialEq,Clone)]
pub struct FullSignature {
    id_version: u8,
    id: PublicSignKey,
    hash_version: u8,
    sig: Sign,
}

impl FullSignature {
    /// Version of the `Hash` used in signature computation.
    pub fn get_hash_version(&self) -> u8 {
        self.hash_version
    }

    /// Version of the `Identity` used in signature computation.
    pub fn get_identity_version(&self) -> u8 {
        self.id_version
    }

    /// Retrieve the Public Key of the signer.
    pub fn signed_by(&self) -> &PublicSignKey {
        &self.id
    }

    /// Verify the signature is authentic.
    pub fn verify(&self, hash: &Hash) -> bool {
        verify_detached(&self.id, hash.as_bytes(), &self.sig)
    }

    /// Write the signature data out to a buffer.
    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.id_version)?;
        wr.write_u8(self.hash_version)?;
        wr.write_all(&self.id.0)?;
        wr.write_all(&self.sig.0)?;
        Ok(())
    }

    /// Read a buffer to reconstruct a signature.
    pub fn read<R: Read>(rd: &mut R) -> Result<FullSignature, CryptoError> {
        let id_version = rd.read_u8().map_err(CryptoError::Io)?;
        let hash_version = rd.read_u8().map_err(CryptoError::Io)?;
        if id_version != 1 || hash_version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut id: PublicSignKey = Default::default();
        let mut sig: Sign = Default::default();
        rd.read_exact(&mut id.0).map_err(CryptoError::Io)?;
        rd.read_exact(&mut sig.0).map_err(CryptoError::Io)?;
        Ok(FullSignature {
            id_version,
            id,
            hash_version,
            sig
        })
    }
}

/// FullKeys are the secret data needed to act as a particular FullIdentity.
/// - Consists of a Ed25519 private/public keypair, as well as the Curve25519 private key
/// - When encoded, it is just the "seed", from which the keypair and Curve25519 key can be derived
/// - The seed is actually the Ed25519 private key
#[derive(Debug,PartialEq,Clone)]
pub struct FullKey {
    version: u8,
    signing: SecretSignKey,
    decrypting: SecretCryptKey,
}

impl FullKey {

    fn blank() -> FullKey {
        FullKey {
            version: 0,
            signing: Default::default(),
            decrypting: Default::default(),
        }
    }

    fn complete(&mut self) {
        ed25519_sk_to_curve25519(&mut self.decrypting, &self.signing)
    }

    pub fn new_pair() -> Result<(FullKey, FullIdentity), CryptoError> {
        let mut key = FullKey::blank();
        let mut id = FullIdentity::blank();
        key.version = 1;
        id.version = 1;
        sign_keypair(&mut id.signing, &mut key.signing);
        key.complete();
        id.complete()?;
        Ok((key, id))
    }

    pub fn from_seed(seed: Seed) -> Result<(FullKey, FullIdentity), CryptoError> {
        let mut key = FullKey::blank();
        let mut id = FullIdentity::blank();
        key.version = 1;
        id.version = 1;
        sign_seed_keypair(&mut id.signing, &mut key.signing, &seed);
        key.complete();
        id.complete()?;
        Ok((key,id))
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_id(&self) -> PublicSignKey {
        let mut pk: PublicSignKey = Default::default();
        ed25519_sk_to_pk(&mut pk, &self.signing);
        pk
    }

    pub fn get_key_ref(&self) -> Key {
        Key {
            version: self.version,
            id: self.get_id(),
        }
    }

    pub fn get_identity(&self) -> Result<FullIdentity, CryptoError> {
        let mut id = FullIdentity::blank();
        id.version = 1;
        ed25519_sk_to_pk(&mut id.signing, &self.signing);
        id.complete()?;
        Ok(id)
    }

    pub fn calc_stream_key(&self, pk: &PublicCryptKey) -> Result<SecretKey, CryptoError> {
        calc_secret(pk, &self.decrypting)
    }

    pub fn sign(&self, hash: &Hash) -> FullSignature {
        FullSignature { 
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

    pub fn read<R: Read>(rd: &mut R) -> Result<(FullKey,FullIdentity), CryptoError> {
        let mut seed: Seed = Default::default();
        let version = rd.read_u8().map_err(CryptoError::Io)?;
        match version {
            1 => {
                rd.read_exact(&mut seed.0).map_err(CryptoError::Io)?;
                FullKey::from_seed(seed)
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct FullIdentity {
    version: u8,                         // Crypto version
    signing: PublicSignKey,       // Version 1: Ed25519 public key
    encrypting: PublicCryptKey, // Version 1: Curve 25519 public key
}

/// Consists of a Ed25519 public key and corresponding Curve25519 public key.
/// When encoded, it is just the Ed25519 public key
impl FullIdentity {

    fn blank() -> FullIdentity {
        FullIdentity {
            version: 0,
            signing: Default::default(),
            encrypting: Default::default(),
        }
    }

    fn complete(&mut self) -> Result<(), CryptoError> {
        ed25519_pk_to_curve25519_pk(&mut self.encrypting, &self.signing)
    }

    pub fn from_public_key(pk: PublicSignKey) -> Result<FullIdentity,CryptoError> {
        let mut id = FullIdentity {
            version: 1,
            signing: pk,
            encrypting: Default::default(),
        };
        id.complete()?;
        Ok(id)
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_id(&self) -> PublicSignKey {
        self.signing.clone()
    }

    pub fn get_identity_ref(&self) -> Identity {
        Identity {
            version: self.version,
            id: self.get_id(),
        }
    }

    pub fn calc_stream_key(&self, sk: &SecretCryptKey) -> Result<SecretKey, CryptoError> {
        calc_secret(&self.encrypting, sk)
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.signing.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<FullIdentity, CryptoError> {
        let mut id = FullIdentity::blank();
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


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::init;

    fn key_enc_dec(k: FullKey) {
        let mut v = Vec::new();
        k.write(&mut v).unwrap();
        let id = k.get_identity().unwrap();
        let (kd,idd) = FullKey::read(&mut &v[..]).unwrap();
        assert_eq!(k, kd);
        assert_eq!(id,idd);
    }

    fn identity_enc_dec(id: FullIdentity) {
        let mut v = Vec::new();
        id.write(&mut v).unwrap();
        let idd = FullIdentity::read(&mut &v[..]).unwrap();
        assert_eq!(id,idd);
    }

    fn signature_enc_dec(sig: FullSignature) {
        let mut v = Vec::new();
        sig.write(&mut v).unwrap();
        let sigd = FullSignature::read(&mut &v[..]).unwrap();
        assert_eq!(sig,sigd);
    }

    #[test]
    fn new() {
        init().unwrap();
        let (k, id) = FullKey::new_pair().unwrap();
        assert_eq!(k.get_version(),1);
        assert_eq!(k.get_id().0, id.get_id().0);
        assert_eq!(k.get_identity().unwrap(), id);
        // Keys shouldn't be 0 (the default)
        assert_ne!(k.get_id(), Default::default());
        assert_ne!(id.get_id(), Default::default());
        key_enc_dec(k);
        identity_enc_dec(id);
    }

    #[test]
    fn stream_key() {
        // We should always get the same shared secrets for all of the below calls
        init().unwrap();
        let (k1, id1) = FullKey::new_pair().unwrap();
        let (k2, id2) = FullKey::new_pair().unwrap();
        let s_k1_id2 = k1.calc_stream_key(&id2.encrypting).unwrap();
        let s_id1_k2 = id1.calc_stream_key(&k2.decrypting).unwrap();
        let s_k2_id1 = k2.calc_stream_key(&id1.encrypting).unwrap();
        let s_id2_k1 = id2.calc_stream_key(&k1.decrypting).unwrap();
        assert_eq!(s_k1_id2, s_id1_k2);
        assert_eq!(s_id1_k2, s_k2_id1);
        assert_eq!(s_k2_id1, s_id2_k1);
    }

    #[test]
    fn key_edge_cases() {
        let v = vec![0_u8];
        let err = FullKey::read(&mut &v[..]).unwrap_err();
        match err {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("FullKey should never decode if version is 0"),
        };
    }

    #[test]
    fn signing() {
        init().unwrap();
        let v: Vec<u8> = b"This is a test".to_vec();
        let h = Hash::new(1, &v[..]).unwrap();
        let (k, id) = FullKey::new_pair().unwrap();
        let sig = k.sign(&h);
        assert_eq!(sig.get_hash_version(), 1);
        assert_eq!(sig.get_identity_version(), 1);
        assert_eq!(*sig.signed_by(), k.get_id());
        assert_eq!(*sig.signed_by(), id.get_id());
        assert!(sig.verify(&h));
        signature_enc_dec(sig);
    }

}
