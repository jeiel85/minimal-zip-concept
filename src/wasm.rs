use crate::cli::{CompressionMode, EntropyMode};
use crate::{compress_bytes_v2_with_progress_dict, decompress_bytes_v2_dict};

#[no_mangle]
pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn wasm_free(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

#[no_mangle]
pub extern "C" fn wasm_compress(
    in_ptr: *const u8,
    in_len: usize,
    mode: u8,
    entropy: u8,
    level: u8,
    delta: u8,
    bcj: u8,
    png: u8,
    lpc: u8,
    bwt: u8,
    dict_ptr: *const u8,
    dict_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let in_bytes = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };
    let dict_bytes = if !dict_ptr.is_null() && dict_len > 0 {
        Some(unsafe { std::slice::from_raw_parts(dict_ptr, dict_len) })
    } else {
        None
    };

    let comp_mode = match mode {
        0 => CompressionMode::Rle,
        1 => CompressionMode::Dict,
        2 => CompressionMode::Hybrid,
        3 => CompressionMode::Lz77,
        _ => CompressionMode::Hybrid,
    };

    let ent_mode = match entropy {
        0 => EntropyMode::None,
        1 => EntropyMode::Huffman,
        2 => EntropyMode::Dynamic,
        3 => EntropyMode::Ans,
        4 => EntropyMode::Cm,
        _ => EntropyMode::Huffman,
    };

    let out_bytes = compress_bytes_v2_with_progress_dict(
        in_bytes,
        comp_mode,
        ent_mode,
        level,
        delta != 0,
        bcj != 0,
        png != 0,
        lpc != 0,
        bwt != 0,
        dict_bytes,
        |_, _, _, _| {},
    );

    let out_len = out_bytes.len();
    unsafe {
        *out_len_ptr = out_len;
    }

    let mut out_vec = out_bytes;
    let ptr = out_vec.as_mut_ptr();
    std::mem::forget(out_vec);
    ptr
}

#[no_mangle]
pub extern "C" fn wasm_decompress(
    in_ptr: *const u8,
    in_len: usize,
    dict_ptr: *const u8,
    dict_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let in_bytes = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };
    let dict_bytes = if !dict_ptr.is_null() && dict_len > 0 {
        Some(unsafe { std::slice::from_raw_parts(dict_ptr, dict_len) })
    } else {
        None
    };

    match decompress_bytes_v2_dict(in_bytes, dict_bytes) {
        Ok(decomp) => {
            let out_len = decomp.len();
            unsafe {
                *out_len_ptr = out_len;
            }
            let mut decomp_vec = decomp;
            let ptr = decomp_vec.as_mut_ptr();
            std::mem::forget(decomp_vec);
            ptr
        }
        Err(_) => {
            unsafe {
                *out_len_ptr = 0;
            }
            std::ptr::null_mut()
        }
    }
}
