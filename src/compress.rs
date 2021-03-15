use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{convert::TryFrom, fmt};

pub const ALGORITHM_ZSTD: u8 = 0;

/// Defines the compression types supported by documents & entries. Format when encoded is a single
/// byte, with the lowest two bits indicating the actual compression type. The upper 6 bits are
/// reserved for possible future compression formats. For now, the only allowed compression is
/// zstd, where the upper 6 bits are 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompressType {
    /// No compression
    NoCompress,
    /// Standard Compression
    Compress,
    /// Dictionary compression
    DictCompress,
}

impl CompressType {
    pub fn type_of(compress: &Compress) -> Self {
        match compress {
            Compress::None => CompressType::NoCompress,
            Compress::General { .. } => CompressType::Compress,
            Compress::Dict(_) => CompressType::DictCompress,
        }
    }
}

impl From<CompressType> for u8 {
    fn from(val: CompressType) -> u8 {
        match val {
            CompressType::NoCompress => 0,
            CompressType::Compress => 1,
            CompressType::DictCompress => 2,
        }
    }
}

impl TryFrom<u8> for CompressType {
    type Error = u8;
    fn try_from(val: u8) -> Result<CompressType, u8> {
        match val {
            0 => Ok(CompressType::NoCompress),
            1 => Ok(CompressType::Compress),
            2 => Ok(CompressType::DictCompress),
            _ => Err(val),
        }
    }
}

/// Compression settings for Documents and Entries.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Compress {
    /// Don't compress by default.
    None,
    /// Compress using the given algorithm identifier and compression level.
    General { algorithm: u8, level: u8 },
    /// Compress using the provided dictionary object
    Dict(Dictionary),
}

impl Compress {
    /// Create a new general Zstd Compression setting.
    pub fn new_zstd_general(level: u8) -> Self {
        Compress::General {
            algorithm: ALGORITHM_ZSTD,
            level,
        }
    }

    /// Create a new ZStandard dictionary with the given compression level.
    pub fn new_zstd_dict(level: u8, dict: Vec<u8>) -> Self {
        Compress::Dict(Dictionary::new_zstd(level, dict))
    }

    /// Attempt to compress the data. Failure occurs if this shouldn't compress, compression fails,
    /// or the result is longer than the original. On failure, the buffer is discarded.
    pub(crate) fn compress(&self, mut dest: Vec<u8>, src: &[u8]) -> Result<Vec<u8>, ()> {
        match self {
            Compress::None => Err(()),
            Compress::General { level, .. } => {
                let dest_len = dest.len();
                let max_len = zstd_safe::compress_bound(src.len());
                dest.reserve(max_len);
                unsafe {
                    dest.set_len(dest_len + max_len);
                    match zstd_safe::compress(&mut dest[dest_len..], src, *level as i32) {
                        Ok(len) if len < src.len() => {
                            dest.truncate(dest_len + len);
                            Ok(dest)
                        }
                        _ => Err(()),
                    }
                }
            }
            Compress::Dict(dict) => {
                let dest_len = dest.len();
                let max_len = zstd_safe::compress_bound(src.len());
                dest.reserve(max_len);
                unsafe {
                    dest.set_len(dest_len + max_len);
                    match &dict.0 {
                        DictionaryPrivate::Unknown { level, .. } => {
                            match zstd_safe::compress(&mut dest[dest_len..], src, *level as i32) {
                                Ok(len) if len < src.len() => {
                                    dest.truncate(dest_len + len);
                                    Ok(dest)
                                }
                                _ => Err(()),
                            }
                        }
                        DictionaryPrivate::Zstd { cdict, .. } => {
                            let mut ctx = zstd_safe::create_cctx();
                            match ctx.compress_using_cdict(&mut dest[dest_len..], src, cdict) {
                                Ok(len) if len < src.len() => {
                                    dest.truncate(dest_len + len);
                                    Ok(dest)
                                }
                                _ => Err(()),
                            }
                        }
                    }
                }
            }
        }
    }

    /// Attempt to decompress the data. Fails if the result in `dest` would be greater than
    /// `max_size`, or if decompression fails.
    pub(crate) fn decompress(
        &self,
        mut dest: Vec<u8>,
        src: &[u8],
        marker: CompressType,
        extra_size: usize,
        max_size: usize,
    ) -> Result<Vec<u8>> {
        match marker {
            CompressType::NoCompress => {
                if dest.len() + src.len() + extra_size > max_size {
                    Err(Error::FailDecompress(format!(
                        "Decompressed length {} would be larger than maximum of {}",
                        dest.len() + src.len() + extra_size,
                        max_size
                    )))
                } else {
                    dest.reserve(src.len() + extra_size);
                    dest.extend_from_slice(src);
                    Ok(dest)
                }
            }
            CompressType::Compress => {
                // Prep for decomressed data
                let header_len = dest.len();
                let expected_len = zstd_safe::get_frame_content_size(src);
                if expected_len > (max_size - header_len) as u64 {
                    return Err(Error::FailDecompress(format!(
                        "Decompressed length {} would be larger than maximum of {}",
                        dest.len() + src.len(),
                        max_size
                    )));
                }
                let expected_len = expected_len as usize;
                dest.reserve(expected_len + extra_size);

                // Safety: Immediately before this, we reserve enough space for the header and the
                // expected length, so setting the length is OK. The decompress function overwrites
                // data and returns the new valid length, so no data is uninitialized after this
                // block completes. In the event of a failure, the vec is freed, so it is never
                // returned in an invalid state.
                unsafe {
                    dest.set_len(header_len + expected_len);
                    let len = zstd_safe::decompress(&mut dest[header_len..], src).map_err(|e| {
                        Error::FailDecompress(format!("Failed Decompression, zstd error = {}", e))
                    })?;
                    dest.truncate(header_len + len);
                }
                Ok(dest)
            }
            CompressType::DictCompress => {
                // Fetch dictionary
                let ddict = if let Compress::Dict(Dictionary(DictionaryPrivate::Zstd {
                    ddict,
                    ..
                })) = self
                {
                    ddict
                } else {
                    return Err(Error::BadHeader(
                            "Header uses dictionary compression, but this has no matching supported dictionary".into()));
                };

                // Prep for decompressed data
                let header_len = dest.len();
                let expected_len = zstd_safe::get_frame_content_size(src);
                if expected_len > (max_size - header_len) as u64 {
                    return Err(Error::FailDecompress(format!(
                        "Decompressed length {} would be larger than maximum of {}",
                        dest.len() + src.len(),
                        max_size
                    )));
                }
                let expected_len = expected_len as usize;
                dest.reserve(expected_len + extra_size);

                // Safety: Immediately before this, we reserve enough space for the header and the
                // expected length, so setting the length is OK. The decompress function overwrites
                // data and returns the new valid length, so no data is uninitialized after this
                // block completes. In the event of a failure, the vec is freed, so it is never
                // returned in an invalid state.
                unsafe {
                    dest.set_len(header_len + expected_len);
                    let mut dctx = zstd_safe::create_dctx();
                    let len = dctx
                        .decompress_using_ddict(&mut dest[header_len..], src, ddict)
                        .map_err(|e| {
                            Error::FailDecompress(format!(
                                "Failed Decompression, zstd error = {}",
                                e
                            ))
                        })?;
                    dest.truncate(header_len + len);
                }
                Ok(dest)
            }
        }
    }
}

impl std::default::Default for Compress {
    fn default() -> Self {
        Compress::General {
            algorithm: ALGORITHM_ZSTD,
            level: 3,
        }
    }
}

/// A ZStandard Compression dictionary.
///
/// A new dictionary can be created by providing the desired compression level and the dictionary
/// as a byte vector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dictionary(DictionaryPrivate);

impl Dictionary {
    /// Create a new ZStandard compression dictionary.
    pub fn new_zstd(level: u8, dict: Vec<u8>) -> Self {
        let cdict = zstd_safe::create_cdict(&dict, level as i32);
        let ddict = zstd_safe::create_ddict(&dict);
        Self(DictionaryPrivate::Zstd {
            level,
            dict,
            cdict,
            ddict,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(from = "DictionarySerde", into = "DictionarySerde")]
enum DictionaryPrivate {
    Unknown {
        algorithm: u8,
        level: u8,
        dict: Vec<u8>,
    },
    Zstd {
        level: u8,
        dict: Vec<u8>,
        cdict: zstd_safe::CDict<'static>,
        ddict: zstd_safe::DDict<'static>,
    },
}

// Struct used solely for serialization/deserialization
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DictionarySerde {
    algorithm: u8,
    level: u8,
    dict: ByteBuf,
}

impl Clone for DictionaryPrivate {
    fn clone(&self) -> Self {
        match self {
            DictionaryPrivate::Unknown {
                algorithm,
                level,
                dict,
            } => DictionaryPrivate::Unknown {
                algorithm: *algorithm,
                level: *level,
                dict: dict.clone(),
            },
            DictionaryPrivate::Zstd { level, dict, .. } => DictionaryPrivate::Zstd {
                level: *level,
                dict: dict.clone(),
                cdict: zstd_safe::create_cdict(dict, *level as i32),
                ddict: zstd_safe::create_ddict(dict),
            },
        }
    }
}

impl fmt::Debug for DictionaryPrivate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let (algorithm, level, dict) = match self {
            DictionaryPrivate::Unknown {
                algorithm,
                level,
                dict,
            } => (algorithm, level, dict),
            DictionaryPrivate::Zstd { level, dict, .. } => (&ALGORITHM_ZSTD, level, dict),
        };
        fmt.debug_struct("Dictionary")
            .field("algorithm", algorithm)
            .field("level", level)
            .field("dict", dict)
            .finish()
    }
}

impl From<DictionarySerde> for DictionaryPrivate {
    fn from(value: DictionarySerde) -> Self {
        match value.algorithm {
            ALGORITHM_ZSTD => {
                let cdict = zstd_safe::create_cdict(&value.dict, value.level as i32);
                let ddict = zstd_safe::create_ddict(&value.dict);
                DictionaryPrivate::Zstd {
                    level: value.level,
                    dict: value.dict.into_vec(),
                    cdict,
                    ddict,
                }
            }
            _ => DictionaryPrivate::Unknown {
                algorithm: value.algorithm,
                level: value.level,
                dict: value.dict.into_vec(),
            },
        }
    }
}

impl From<DictionaryPrivate> for DictionarySerde {
    fn from(value: DictionaryPrivate) -> Self {
        match value {
            DictionaryPrivate::Unknown {
                algorithm,
                level,
                dict,
            } => Self {
                algorithm,
                level,
                dict: ByteBuf::from(dict),
            },
            DictionaryPrivate::Zstd { level, dict, .. } => Self {
                algorithm: ALGORITHM_ZSTD,
                level,
                dict: ByteBuf::from(dict),
            },
        }
    }
}
