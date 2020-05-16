use zstd_safe::*;
use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;


pub fn compress(cctx: &mut CCtx, level: i32, raw: &[u8], buf: &mut Vec<u8>) {
    let vec_len = buf.len();
    let mut buffer_len = compress_bound(raw.len());
    buf.reserve(buffer_len);
    unsafe {
        buf.set_len(vec_len + buffer_len);
        buffer_len = compress_cctx(
            cctx,
            &mut buf[vec_len..],
            raw,
            level
        ).expect("zstd library unexpectedly errored during compress_cctx!");
        buf.set_len(vec_len + buffer_len);
    }
}


pub fn decompress(dctx: &mut DCtx, max_size: usize, buf: &[u8], decode: &mut Vec<u8>) -> io::Result<()> {
    // Decompress the data
    // Find the expected size, and fail if it's larger than the maximum allowed size.
    let decode_len = decode.len();
    let expected_len = get_frame_content_size(buf);
    if ((decode_len as u64)+expected_len) > (max_size as u64) {
        return Err(Error::new(InvalidData, "Expected decompressed size is larger than maximum allowed size"));
    }
    let expected_len = expected_len as usize;
    decode.reserve(expected_len);
    unsafe {
        decode.set_len(decode_len + expected_len);
        let len = decompress_dctx(
            dctx,
            &mut decode[..],
            buf
        ).map_err(|_| Error::new(InvalidData, "Decompression failed"))?;
        decode.set_len(decode_len + len);
    }
    Ok(())
}

pub fn dict_compress(cctx: &mut CCtx, dict: &CDict, raw: &[u8], buf: &mut Vec<u8>) {
    let vec_len = buf.len();
    let mut buffer_len = zstd_safe::compress_bound(raw.len());
    buf.reserve(buffer_len);
    unsafe {
        buf.set_len(vec_len + buffer_len);
        buffer_len = zstd_safe::compress_using_cdict(
            cctx,
            &mut buf[vec_len..],
            raw,
            dict
        ).expect("zstd library unexpectedly errored during compress_cctx!");
        buf.set_len(vec_len + buffer_len);
    }
}

pub fn dict_decompress(dctx: &mut DCtx, dict: &DDict, max_size: usize, buf: &[u8], decode: &mut Vec<u8>) -> io::Result<()> {
    // Decompress the data
    // Find the expected size, and fail if it's larger than the maximum allowed size.
    let decode_len = decode.len();
    let expected_len = get_frame_content_size(buf);
    if ((decode_len as u64)+expected_len) > (max_size as u64) {
        return Err(Error::new(InvalidData, "Expected decompressed size is larger than maximum allowed size"));
    }
    let expected_len = expected_len as usize;
    decode.reserve(expected_len);
    unsafe {
        decode.set_len(decode_len + expected_len);
        let len = decompress_using_ddict(
            dctx,
            &mut decode[..],
            buf,
            dict
        ).map_err(|_| Error::new(InvalidData, "Decompression failed"))?;
        decode.set_len(decode_len + len);
    }
    Ok(())
}

