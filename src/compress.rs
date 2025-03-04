use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{cell::RefCell, convert::TryFrom, fmt};

thread_local! {
    static ZSTD_CCTX: RefCell<zstd_safe::CCtx<'static>> = RefCell::new(zstd_safe::CCtx::create());
    static ZSTD_DCTX: RefCell<zstd_safe::DCtx<'static>> = RefCell::new(zstd_safe::DCtx::create());
}

#[derive(Debug, Clone)]
pub enum CompressionError {
    ExceededSize { max: usize, actual: usize },
    ZstdInner(usize),
    Parsing(&'static str),
}

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressionError::ExceededSize { max, actual } => write!(
                f,
                "Decompressed size is {} bytes, larger than max of {} kiB",
                actual,
                (max + 1) >> 10
            ),
            CompressionError::ZstdInner(v) => {
                // SAFETY: We assume the zstd library will always return a valid
                // static C string from this function, as it promises to do.
                let e_str = unsafe {
                    core::ffi::CStr::from_ptr(zstd_safe::zstd_sys::ZSTD_getErrorName(*v))
                };
                let e_str = e_str.to_str().unwrap_or("Undisplayable error code");
                write!(f, "zstd failure, code {} ({})", v, e_str)
            }
            CompressionError::Parsing(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for CompressionError {}

/// The compression algorithm identifier for `zstandard`.
pub const ALGORITHM_ZSTD: u8 = 0;

/// Defines the compression types supported by documents & entries. Format when encoded is a single
/// byte, with the lowest two bits indicating the actual compression type. The upper 6 bits are
/// reserved for possible future compression formats. For now, the only allowed compression is
/// zstd, where the upper 6 bits are 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompressType {
    /// No compression
    None,
    /// Standard Compression
    General,
    /// Dictionary compression
    Dict,
}

impl CompressType {
    pub fn type_of(compress: &Compress) -> Self {
        match compress {
            Compress::None => CompressType::None,
            Compress::General { .. } => CompressType::General,
            Compress::Dict(_) => CompressType::Dict,
        }
    }
}

impl From<CompressType> for u8 {
    fn from(val: CompressType) -> u8 {
        match val {
            CompressType::None => 0,
            CompressType::General => 1,
            CompressType::Dict => 2,
        }
    }
}

impl TryFrom<u8> for CompressType {
    type Error = u8;
    fn try_from(val: u8) -> Result<CompressType, u8> {
        match val {
            0 => Ok(CompressType::None),
            1 => Ok(CompressType::General),
            2 => Ok(CompressType::Dict),
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
    General {
        /// The algorithm's identifier
        algorithm: u8,
        /// The compression level
        level: u8,
    },
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
    pub fn new_zstd_dict(level: u8, dict: Vec<u8>) -> Option<Self> {
        Some(Compress::Dict(Dictionary::new_zstd(level, dict)?))
    }

    /// Attempt to compress the data. Failure occurs if this shouldn't compress, compression fails,
    /// or the result is longer than the original. On failure, the buffer is discarded.
    pub(crate) fn compress(
        &self,
        mut dst: Vec<u8>,
        src: &[u8],
    ) -> Result<Option<Vec<u8>>, CompressionError> {
        match self {
            Compress::None => Ok(None),
            Compress::General { level, .. } => {
                zstd_compress(src, &mut dst, *level as i32, None)?;
                Ok(Some(dst))
            }
            Compress::Dict(dict) => match &dict.0 {
                DictionaryPrivate::Unknown { level, .. } => {
                    zstd_compress(src, &mut dst, *level as i32, None)?;
                    Ok(Some(dst))
                }
                DictionaryPrivate::Zstd { level, cdict, .. } => {
                    zstd_compress(src, &mut dst, *level as i32, Some(cdict))?;
                    Ok(Some(dst))
                }
            },
        }
    }

    /// Attempt to decompress the data. Fails if the result in `dst` would be greater than
    /// `max_size`, or if decompression fails.
    pub(crate) fn decompress(
        &self,
        mut dst: Vec<u8>,
        src: &[u8],
        marker: CompressType,
        extra_size: usize,
        max_size: usize,
    ) -> Result<Vec<u8>> {
        match marker {
            CompressType::None => {
                let size = dst.len() + src.len() + extra_size;
                if size > max_size {
                    Err(Error::LengthTooLong {
                        max: max_size,
                        actual: size,
                    })
                } else {
                    dst.reserve(src.len() + extra_size);
                    dst.extend_from_slice(src);
                    Ok(dst)
                }
            }
            CompressType::General => {
                zstd_decompress(src, &mut dst, None, extra_size, max_size)?;
                Ok(dst)
            }
            CompressType::Dict => {
                // Fetch dictionary
                let Compress::Dict(Dictionary(DictionaryPrivate::Zstd { ddict, .. })) = self else {
                    return Err(Error::BadHeader(
                            "Header uses dictionary compression, but this has no matching supported dictionary".into()));
                };
                zstd_decompress(src, &mut dst, Some(ddict), extra_size, max_size)?;
                Ok(dst)
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
    pub fn new_zstd(level: u8, dict: Vec<u8>) -> Option<Self> {
        let cdict = zstd_safe::CDict::try_create(&dict, level as i32)?;
        let ddict = zstd_safe::DDict::try_create(&dict)?;
        Some(Self(DictionaryPrivate::Zstd {
            level,
            dict,
            cdict,
            ddict,
        }))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(try_from = "DictionarySerde", into = "DictionarySerde")]
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
                cdict: zstd_safe::CDict::create(dict, *level as i32),
                ddict: zstd_safe::DDict::create(dict),
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

impl TryFrom<DictionarySerde> for DictionaryPrivate {
    type Error = &'static str;
    fn try_from(value: DictionarySerde) -> Result<Self, Self::Error> {
        match value.algorithm {
            ALGORITHM_ZSTD => {
                let err = "Invalid ZSTD dictionary";
                let cdict =
                    zstd_safe::CDict::try_create(&value.dict, value.level as i32).ok_or(err)?;
                let ddict = zstd_safe::DDict::try_create(&value.dict).ok_or(err)?;
                Ok(DictionaryPrivate::Zstd {
                    level: value.level,
                    dict: value.dict.into_vec(),
                    cdict,
                    ddict,
                })
            }
            _ => Ok(DictionaryPrivate::Unknown {
                algorithm: value.algorithm,
                level: value.level,
                dict: value.dict.into_vec(),
            }),
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

/// Attempt to train a zstd dictionary
pub fn train_dictionary(
    target_level: u8,
    target_len: usize,
    samples: &[u8],
    sample_lens: &[usize],
) -> Result<Dictionary, CompressionError> {
    let mut dict: Vec<u8> = Vec::with_capacity(target_len);

    unsafe {
        let target = core::slice::from_raw_parts_mut(
            dict.spare_capacity_mut().as_mut_ptr() as *mut u8,
            target_len,
        );
        let dict_len = zstd_safe::train_from_buffer(target, samples, sample_lens)?;
        dict.set_len(dict_len);
        dict[4..8].fill(0);
    }
    let err = "Couldn't parse completed dictionary";
    let cdict = zstd_safe::CDict::try_create(&dict, target_level as i32)
        .ok_or(CompressionError::Parsing(err))?;
    let ddict = zstd_safe::DDict::try_create(&dict).ok_or(CompressionError::Parsing(err))?;

    Ok(Dictionary(DictionaryPrivate::Zstd {
        level: target_level,
        dict,
        cdict,
        ddict,
    }))
}

fn decompressed_size(header: &[u8]) -> Result<usize, CompressionError> {
    let Some(descriptor) = header.first() else {
        return Err(CompressionError::Parsing("not enough bytes in header"));
    };
    if descriptor & 0x1F != 0 {
        return Err(CompressionError::Parsing(
            "Incorrect zstd frame header descriptor for fog-pack",
        ));
    }
    // Fail if the decompressed size wasn't included
    if descriptor & 0xE0 == 0xE0 {
        return Err(CompressionError::Parsing("Missing frame content size"));
    }
    let len = descriptor >> 6;
    let len_offset = 1 + ((descriptor & 0x20 == 0) as usize);
    if header.len() < ((1 << len) as usize + len_offset) {
        return Err(CompressionError::Parsing("Header isn't large enough"));
    }

    // SAFETY:
    // We just verified that the header slice is large enough for these
    // unaligned read operations.
    unsafe {
        let header = header.as_ptr().byte_add(len_offset);
        match len {
            0 => Ok(header.read() as usize),
            1 => Ok((header as *const u16).read_unaligned() as usize + 256),
            2 => {
                let size = (header as *const u32).read_unaligned() as usize;
                if size < 65792 {
                    Err(CompressionError::Parsing(
                        "Didn't use minimal-length encoding of decompressed frame size",
                    ))
                } else {
                    Ok(size)
                }
            }
            _ => Err(CompressionError::Parsing("length too long")),
        }
    }
}

impl From<zstd_safe::ErrorCode> for CompressionError {
    fn from(value: zstd_safe::ErrorCode) -> Self {
        CompressionError::ZstdInner(value)
    }
}

fn zstd_compress(
    input: &[u8],
    output: &mut Vec<u8>,
    level: i32,
    dict: Option<&zstd_safe::CDict<'static>>,
) -> Result<usize, CompressionError> {
    use zstd_safe::*;
    ZSTD_CCTX.with_borrow_mut(|ctx| {
        // Configure the context for our single-frame, minimal-size header.
        ctx.reset(ResetDirective::SessionAndParameters)?;
        if let Some(dict) = dict {
            ctx.ref_cdict(dict)?;
        } else {
            ctx.set_parameter(CParameter::CompressionLevel(level))?;
        }
        ctx.set_parameter(CParameter::DictIdFlag(false))?;
        ctx.set_parameter(CParameter::ChecksumFlag(false))?;
        ctx.set_parameter(CParameter::Format(zstd_safe::FrameFormat::Magicless))?;
        ctx.set_parameter(CParameter::ContentSizeFlag(true))?;
        ctx.set_parameter(CParameter::WindowLog(21))?;
        ctx.set_pledged_src_size(Some(input.len() as u64))?;

        // Reserve space for the output
        output.reserve(compress_bound(input.len()));
        let out_buffer = output.spare_capacity_mut();

        // Perform compression
        let used_len = unsafe {
            let out_buffer = core::slice::from_raw_parts_mut(
                out_buffer.as_mut_ptr() as *mut u8,
                out_buffer.len(),
            );
            let used_len = ctx.compress2(out_buffer, input)?;
            output.set_len(used_len + output.len());
            used_len
        };

        Ok(used_len)
    })
}

fn zstd_decompress(
    input: &[u8],
    output: &mut Vec<u8>,
    dict: Option<&zstd_safe::DDict<'static>>,
    extra_size: usize,
    max_size: usize,
) -> Result<usize, CompressionError> {
    use zstd_safe::*;

    // Reserve space for the output
    let out_size = decompressed_size(input)?;
    let final_size = out_size + extra_size + output.len();
    if final_size > max_size {
        return Err(CompressionError::ExceededSize {
            max: max_size,
            actual: final_size,
        });
    }
    output.reserve(out_size + extra_size);

    ZSTD_DCTX.with_borrow_mut(|dtx| {
        // Set up decompression parameters
        dtx.reset(ResetDirective::SessionAndParameters)?;
        if let Some(dict) = dict {
            dtx.ref_ddict(dict)?;
        };
        dtx.set_parameter(DParameter::Format(FrameFormat::Magicless))?;
        dtx.set_parameter(DParameter::WindowLogMax(21))?;

        // Buffer manipulation, then decompress
        // SAFETY:
        // We're just passing the spare capacity directly to zstd to fill out,
        // then adjusting the vec up by how much zstd filled in.
        let out_buffer = output.spare_capacity_mut();
        let used_len = unsafe {
            let out_buffer = core::slice::from_raw_parts_mut(
                out_buffer.as_mut_ptr() as *mut u8,
                out_buffer.len(),
            );
            let used_len = dtx.decompress(out_buffer, input)?;
            output.set_len(used_len + output.len());
            used_len
        };
        if used_len != out_size {
            return Err(CompressionError::Parsing(
                "Decompressed size doesn't match promised size",
            ));
        }

        Ok(used_len)
    })
}
