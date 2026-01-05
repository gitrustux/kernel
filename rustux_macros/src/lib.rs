// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Procedural macros for the Rustux kernel test framework

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Mark a function as a test case
///
/// This attribute marks a function to be included in the test suite.
/// The function must return `TestResult`.
///
/// # Example
///
/// ```rust
/// #[test_case]
/// fn test_example() -> TestResult {
///     assert_eq!(1 + 1, 2);
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn test_case(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();

    // Extract doc comments for description
    let mut description = String::new();
    for attr in &input.attrs {
        if attr.path().is_ident("doc") {
            if let Ok(doc) = attr.meta.require_list() {
                let doc_str = doc.tokens.to_string();
                if doc_str.starts_with('"') {
                    let trimmed = doc_str.trim_matches('"').to_string();
                    if !trimmed.is_empty() {
                        description = trimmed;
                    }
                }
            }
        }
    }

    let expanded = quote! {
        #[allow(non_camel_case_types)]
        #[cfg(test)]
        fn #fn_name() -> crate::kernel::tests::runner::TestResult {
            #input
        }

        #[cfg(test)]
        #[doc(hidden)]
        const #fn_name: crate::kernel::tests::runner::TestCase =
            crate::kernel::tests::runner::TestCase::new(
                #fn_name_str,
                &#description,
                #fn_name,
            );
    };

    TokenStream::from(expanded)
}

/// Derive macro for test registration
///
/// This is a placeholder for future derive macros that might
/// be needed for test registration.
#[proc_macro_derive(TestRegistration)]
pub fn test_registration_derive(_input: TokenStream) -> TokenStream {
    // Placeholder implementation
    TokenStream::new()
}
