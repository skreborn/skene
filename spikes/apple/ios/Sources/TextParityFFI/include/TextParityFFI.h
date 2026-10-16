#ifndef TEXT_PARITY_FFI_H
#define TEXT_PARITY_FFI_H

/// Runs the apple-parity suite configured by JSON; free the answer with apple_parity_free.
char *apple_parity_run(const char *config);

/// The language line-height probe as JSON; free the answer with apple_parity_free.
char *apple_parity_language(void);

/// Frees a string the functions above answered.
void apple_parity_free(char *text);

/// Keeps the module non-empty for SwiftPM.
int text_parity_ffi_linked(void);

#endif
