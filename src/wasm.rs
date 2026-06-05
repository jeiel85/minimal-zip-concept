use crate::cli::{CompressionMode, EntropyMode};
use crate::{compress_bytes_v2_with_progress_dict_password, decompress_bytes_v2_dict_password};

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
    password_ptr: *const u8,
    password_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let in_bytes = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };
    let dict_bytes = if !dict_ptr.is_null() && dict_len > 0 {
        Some(unsafe { std::slice::from_raw_parts(dict_ptr, dict_len) })
    } else {
        None
    };

    let password_str = if !password_ptr.is_null() && password_len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(password_ptr, password_len) };
        std::str::from_utf8(bytes).ok()
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

    let out_bytes = compress_bytes_v2_with_progress_dict_password(
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
        password_str,
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
    password_ptr: *const u8,
    password_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let in_bytes = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };
    let dict_bytes = if !dict_ptr.is_null() && dict_len > 0 {
        Some(unsafe { std::slice::from_raw_parts(dict_ptr, dict_len) })
    } else {
        None
    };

    let password_str = if !password_ptr.is_null() && password_len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(password_ptr, password_len) };
        std::str::from_utf8(bytes).ok()
    } else {
        None
    };

    match decompress_bytes_v2_dict_password(in_bytes, dict_bytes, password_str) {
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

#[no_mangle]
pub extern "C" fn wasm_list_mzar_files(
    in_ptr: *const u8,
    in_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let in_bytes = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };

    match crate::archive::parse_mzar_metadata(in_bytes) {
        Ok(entries) => {
            if let Ok(json_str) = serde_json::to_string(&entries) {
                let out_len = json_str.len();
                unsafe {
                    *out_len_ptr = out_len;
                }
                let mut out_vec = json_str.into_bytes();
                let ptr = out_vec.as_mut_ptr();
                std::mem::forget(out_vec);
                ptr
            } else {
                unsafe {
                    *out_len_ptr = 0;
                }
                std::ptr::null_mut()
            }
        }
        Err(_) => {
            unsafe {
                *out_len_ptr = 0;
            }
            std::ptr::null_mut()
        }
    }
}
