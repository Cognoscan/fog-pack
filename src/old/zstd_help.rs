use zstd_safe::*;
use Error;

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


pub fn decompress(dctx: &mut DCtx, max_size: usize, extra_size: usize, buf: &[u8], decode: &mut Vec<u8>) -> crate::Result<()> {
    // Decompress the data
    // Find the expected size, and fail if it's larger than the maximum allowed size.
    let decode_len = decode.len();
    let expected_len = get_frame_content_size(buf);
    // First check if expected_len is above size on its own
    if expected_len >= (max_size as u64) {
        return Err(Error::BadSize);
    }
    if (decode_len+extra_size+(expected_len as usize)) >= max_size {
        return Err(Error::BadSize);
    }
    let expected_len = expected_len as usize;
    decode.reserve(expected_len);
    unsafe {
        decode.set_len(decode_len + expected_len);
        let len = decompress_dctx(
            dctx,
            &mut decode[decode_len..],
            buf
        ).map_err(|_| Error::FailDecompress)?;
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

pub fn dict_decompress(dctx: &mut DCtx, dict: &DDict, max_size: usize, extra_size: usize, buf: &[u8], decode: &mut Vec<u8>) -> crate::Result<()> {
    // Decompress the data
    // Find the expected size, and fail if it's larger than the maximum allowed size.
    let decode_len = decode.len();
    let expected_len = get_frame_content_size(buf);
    if expected_len >= (max_size as u64) {
        return Err(Error::BadSize);
    }
    if (decode_len+extra_size+(expected_len as usize)) >= max_size {
        return Err(Error::BadSize);
    }
    let expected_len = expected_len as usize;
    decode.reserve(expected_len);
    unsafe {
        decode.set_len(decode_len + expected_len);
        let len = decompress_using_ddict(
            dctx,
            &mut decode[decode_len..],
            buf,
            dict
        ).map_err(|_| Error::FailDecompress)?;
        decode.set_len(decode_len + len);
    }
    Ok(())
}

pub fn train_dict(dict_size: usize, samples: Vec<Vec<u8>>) -> Result<Vec<u8>, usize> {
    let sizes = samples.iter().map(|x| x.len()).collect::<Vec<usize>>();
    let mut buffer = Vec::with_capacity(sizes.iter().sum());
    for sample in samples.iter() {
        buffer.extend_from_slice(sample);
    }

    let mut dict = vec![0u8; dict_size];
    match zstd_safe::train_from_buffer(&mut dict[..], &buffer[..], &sizes[..]) {
        Ok(size) => {
            dict.resize(size, 0u8);
            Ok(dict)
        },
        Err(e) => {
            Err(e)
        }
    }
}


