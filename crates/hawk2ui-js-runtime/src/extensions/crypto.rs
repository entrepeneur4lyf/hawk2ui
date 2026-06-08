//! Native-safe Web Crypto primitives for the embedded JavaScript runtime.

use deno_core::{Extension, op2};
use deno_error::JsErrorBox;
use serde::Deserialize;
use sha2::{Digest, Sha256};

deno_core::extension!(
    hawk_crypto,
    ops = [op_hawk_crypto_get_random_values, op_hawk_crypto_digest],
);

/// Creates the crypto extension for one runtime instance.
pub(crate) fn extension() -> Extension {
    hawk_crypto::init()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CryptoDigestInput {
    algorithm: String,
    bytes: Vec<u8>,
}

#[op2]
#[serde]
fn op_hawk_crypto_get_random_values(#[smi] length: u32) -> Result<Vec<u8>, JsErrorBox> {
    if length > 65_536 {
        return Err(JsErrorBox::generic(
            "js-runtime.crypto.invalid: crypto.getRandomValues is limited to 65536 bytes",
        ));
    }

    let mut bytes = vec![0_u8; length as usize];
    getrandom::fill(&mut bytes).map_err(|error| {
        JsErrorBox::generic(format!(
            "js-runtime.crypto.random-failed: native random source failed: {error}"
        ))
    })?;
    Ok(bytes)
}

#[op2]
#[serde]
fn op_hawk_crypto_digest(#[serde] input: CryptoDigestInput) -> Result<Vec<u8>, JsErrorBox> {
    match normalized_algorithm(&input.algorithm).as_str() {
        "sha-256" => Ok(Sha256::digest(input.bytes).to_vec()),
        algorithm => Err(JsErrorBox::generic(format!(
            "js-runtime.crypto.unsupported: crypto.subtle.digest does not support {algorithm}"
        ))),
    }
}

fn normalized_algorithm(algorithm: &str) -> String {
    algorithm.trim().to_ascii_lowercase()
}
