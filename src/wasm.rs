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
        None,
        0,
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

fn parse_raw_mzar_to_entries(bytes: &[u8]) -> Result<Vec<crate::archive::MzarEntry>, String> {
    if bytes.len() < 8 {
        return Err("MZAR data too short".to_string());
    }
    if &bytes[0..4] != b"MZAR" {
        return Err("Not a valid MZAR archive".to_string());
    }
    let entry_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mut cursor = 8;
    let mut entries = Vec::new();
    for _ in 0..entry_count {
        if cursor + 2 > bytes.len() {
            return Err("Unexpected EOF".to_string());
        }
        let path_len = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        if cursor + path_len > bytes.len() {
            return Err("Unexpected EOF reading path".to_string());
        }
        let path = std::str::from_utf8(&bytes[cursor..cursor + path_len])
            .map_err(|e| e.to_string())?
            .to_string();
        cursor += path_len;
        if cursor + 9 > bytes.len() {
            return Err("Unexpected EOF reading metadata".to_string());
        }
        let entry_type = bytes[cursor];
        cursor += 1;
        let size = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap()) as usize;
        cursor += 8;
        if cursor + size > bytes.len() {
            return Err("Unexpected EOF reading entry data".to_string());
        }
        let data = bytes[cursor..cursor + size].to_vec();
        cursor += size;

        entries.push(crate::archive::MzarEntry {
            relative_path: path,
            entry_type,
            size: size as u64,
            data,
        });
    }
    Ok(entries)
}

#[no_mangle]
pub extern "C" fn wasm_compress_v2(
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
    solid: u8,
    chunk_size: u32,
    checksum_type: u8,
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

    let chunk_size_opt = if chunk_size > 0 {
        Some(chunk_size)
    } else {
        None
    };

    let final_bytes = if solid == 0 {
        match parse_raw_mzar_to_entries(in_bytes) {
            Ok(entries) => {
                let params = crate::archive::CompressionParams {
                    mode: comp_mode,
                    entropy: ent_mode,
                    level,
                    delta: delta != 0,
                    bcj: bcj != 0,
                    png: png != 0,
                    lpc: lpc != 0,
                    bwt: bwt != 0,
                    dict_data: dict_bytes,
                    password: password_str,
                    chunk_size: chunk_size_opt,
                    checksum_type,
                };
                crate::archive::serialize_entries_custom(entries, Some(&params))
                    .unwrap_or_else(|_| Vec::new())
            }
            Err(_) => Vec::new(),
        }
    } else {
        compress_bytes_v2_with_progress_dict_password(
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
            chunk_size_opt,
            checksum_type,
            |_, _, _, _| {},
        )
    };

    let out_len = final_bytes.len();
    unsafe {
        *out_len_ptr = out_len;
    }

    let mut out_vec = final_bytes;
    let ptr = out_vec.as_mut_ptr();
    std::mem::forget(out_vec);
    ptr
}

#[no_mangle]
pub extern "C" fn wasm_extract_single_file_from_mzar(
    mzar_ptr: *const u8,
    mzar_len: usize,
    path_ptr: *const u8,
    path_len: usize,
    password_ptr: *const u8,
    password_len: usize,
    dict_ptr: *const u8,
    dict_len: usize,
    out_len_ptr: *mut usize,
) -> *mut u8 {
    let mzar_bytes = unsafe { std::slice::from_raw_parts(mzar_ptr, mzar_len) };

    let path_str = if !path_ptr.is_null() && path_len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
        std::str::from_utf8(bytes).unwrap_or("")
    } else {
        ""
    };

    let password_str = if !password_ptr.is_null() && password_len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(password_ptr, password_len) };
        std::str::from_utf8(bytes).ok()
    } else {
        None
    };

    let dict_bytes = if !dict_ptr.is_null() && dict_len > 0 {
        Some(unsafe { std::slice::from_raw_parts(dict_ptr, dict_len) })
    } else {
        None
    };

    match crate::archive::extract_single_file_from_mzar(
        mzar_bytes,
        path_str,
        password_str,
        dict_bytes,
    ) {
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
