#pragma once

#include <cstdint>
#include <cstddef>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PaaFfiHandle PaaFfiHandle;

PaaFfiHandle* paa_ffi_load(const char* path);
void paa_ffi_free(PaaFfiHandle* handle);
uint32_t paa_ffi_get_width(PaaFfiHandle* handle);
uint32_t paa_ffi_get_height(PaaFfiHandle* handle);
bool paa_ffi_is_pot(PaaFfiHandle* handle);
char* paa_ffi_get_format_name(PaaFfiHandle* handle);
uint32_t paa_ffi_get_mipmap_count(PaaFfiHandle* handle);
bool paa_ffi_get_mipmap_info(
    PaaFfiHandle* handle,
    uint32_t index,
    uint32_t* out_width,
    uint32_t* out_height,
    size_t* out_comp_size,
    size_t* out_uncomp_size,
    bool* out_is_compressed
);
bool paa_ffi_decode_mipmap_rgba(PaaFfiHandle* handle, uint32_t index, uint8_t* out_buffer, size_t buffer_len);
bool paa_ffi_get_colors(PaaFfiHandle* handle, uint8_t* out_avg_rgba, uint8_t* out_max_rgba, bool* out_has_alpha);
char* paa_ffi_get_json_metadata(PaaFfiHandle* handle);
void paa_ffi_free_string(char* s);
char* paa_ffi_convert(const char* input_path, const char* output_path, const char* format_name, const char* resize_str, bool generate_mipmaps);

#ifdef __cplusplus
}
#endif
