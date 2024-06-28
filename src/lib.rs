#![warn(clippy::just_underscores_and_digits)]
use ecc::ecc_signable::Signature;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use tiny_ed448_goldilocks::curve::extended_edwards::ExtendedPoint;

const RATE_IN_BYTES: usize = 136; // SHA3-256 r = 1088 / 8 = 136

#[cfg(test)]
const NIST_DATA_SPONGE_INIT: [u8; 200] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
    0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x5b, 0x5c, 0x5d, 0x5e, 0x5f,
    0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f,
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f,
    0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
    0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
    0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
];

/// A simple error type
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum OperationError {
    UnsupportedSecurityParameter,
    CShakeError,
    KmacError,
    SignatureVerificationFailure,
    SHA3DecryptionFailure,
    KeyDecryptionError,
    EmptyDecryptionError,
    DigestNotSet,
    SymNonceNotSet,
    SecurityParameterNotSet,
    XORFailure,
    BytesToScalarError,
    OperationResultNotSet,
    SignatureNotSet,
    UnsupportedCapacity,
    AESCTRDecryptionFailure,
    SecretNotSet,
    InvalidSecretLength,
    DecapsulationFailure,
}

pub mod ecc;
pub mod mlkem;

/// Module for SHA-3 primitives
pub mod sha3 {

    /// Submodule that implements NIST 800-185 compliant functions
    pub mod aux_functions;

    /// Submodule that implements the Keccak-f[1600] permutation
    pub mod keccakf;

    pub mod sha3_hashable;
    pub mod shake_functions;
    /// Submodule that implements the sponge construction
    pub mod sponge;
    pub mod sponge_encryptable;
}

pub mod aes {
    pub mod aes_constants;
    pub mod aes_encryptable;
    pub mod aes_functions;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
/// Message struct for which cryptographic traits are defined.
pub struct Message {
    /// Input message
    pub msg: Box<Vec<u8>>,
    /// The digest lengths in FIPS-approved hash functions
    pub d: Option<SecParam>,
    /// Nonce used in symmetric encryption
    pub sym_nonce: Option<Vec<u8>>,
    /// Nonce used in asymmetric encryption
    pub asym_nonce: Option<ExtendedPoint>,
    /// Hash value (also known as message digest)
    pub digest: Result<Vec<u8>, OperationError>,
    /// Result of the cryptographic trait
    pub op_result: Result<(), OperationError>,
    /// Schnorr signatures on the input message
    pub sig: Option<Signature>,
    /// ML-KEM encrypted secret as a byte array
    pub kem_ciphertext: Option<Vec<u8>>,
}

impl Message {
    /// Returns a new empty Message instance
    pub fn new(data: Vec<u8>) -> Message {
        Message {
            msg: Box::new(data),
            d: None,
            sym_nonce: None,
            asym_nonce: None,
            digest: Ok(vec![]),
            op_result: Ok(()),
            sig: None,
            kem_ciphertext: Some(vec![]),
        }
    }

    pub fn write_to_file(&self, filename: &str) -> std::io::Result<()> {
        let json_key_pair = serde_json::to_string(self).unwrap();
        std::fs::write(filename, json_key_pair)
    }

    pub fn read_from_file(filename: &str) -> Result<Message, Box<dyn std::error::Error>> {
        let mut file = File::open(filename)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let message: Message = serde_json::from_str(&contents)?;
        Ok(message)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
/// An enum representing standard digest lengths based on FIPS PUB 202
pub enum SecParam {
    /// Digest length of 224 bits, also known as SHA3-224
    D224 = 224,
    /// Digest length of 256 bits, also known as SHA3-256
    D256 = 256,
    /// Digest length of 384 bits, also known as SHA3-384
    D384 = 384,
    /// Digest length of 512 bits, also known as SHA3-512
    D512 = 512,
}

impl SecParam {
    /// Converts an integer input to the corresponding security parameter.
    /// Supports security levels of 224, 256, 384, and 512 bits.
    pub fn from_int(value: usize) -> Result<SecParam, OperationError> {
        match value {
            224 => Ok(SecParam::D224),
            256 => Ok(SecParam::D256),
            384 => Ok(SecParam::D384),
            512 => Ok(SecParam::D512),
            _ => Err(OperationError::UnsupportedSecurityParameter),
        }
    }

    fn bytepad_value(&self) -> u32 {
        match self {
            SecParam::D224 => 172,
            SecParam::D256 => 168,
            SecParam::D384 => 152,
            SecParam::D512 => 136,
        }
    }

    pub fn validate(&self) -> Result<(), OperationError> {
        match self {
            SecParam::D224 | SecParam::D256 | SecParam::D384 | SecParam::D512 => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// An enum representing standard capacity valuess based on FIPS PUB 202.
/// (The capacity of a sponge function) = 2 * (digest length)
pub(crate) enum Capacity {
    /// 2 * SecParam.D224
    C448 = 448,
    /// 2 * SecParam.D256
    C512 = 512,
    /// 2 * SecParam.D384
    C768 = 768,
    /// 2 * SecParam.D512
    C1024 = 1024,
}

impl Capacity {
    /// This function effectively maps a given bit length to the appropriate capacity value enum variant,
    fn from_bit_length(bit_length: u64) -> Self {
        match bit_length * 2 {
            x if x <= 448 => Capacity::C448,
            x if x <= 512 => Capacity::C512,
            x if x <= 768 => Capacity::C768,
            _ => Capacity::C1024,
        }
    }
}

/// OutputLength struct for storing the output length.
pub struct OutputLength {
    value: u64,
}

impl OutputLength {
    const MAX_VALUE: u64 = u64::MAX;

    pub fn try_from(value: u64) -> Result<Self, OperationError> {
        if value < Self::MAX_VALUE {
            Ok(OutputLength { value })
        } else {
            Err(OperationError::UnsupportedSecurityParameter)
        }
    }

    pub fn value(&self) -> u64 {
        self.value
    }
}

/// Rate struct for storing the rate value.
/// Rate is the number of input bits processed per invocation of the underlying function in sponge construction.
pub struct Rate {
    value: u64,
}

impl Rate {
    /// Rate = (Permutation width) - (Capacity)
    pub fn from<R: BitLength + ?Sized>(sec_param: &R) -> Self {
        Rate {
            value: (1600 - sec_param.bit_length()),
        }
    }

    pub fn value(&self) -> u64 {
        self.value
    }
}

pub trait BitLength {
    fn bit_length(&self) -> u64;
}

impl BitLength for Capacity {
    fn bit_length(&self) -> u64 {
        *self as u64
    }
}

impl BitLength for SecParam {
    fn bit_length(&self) -> u64 {
        *self as u64
    }
}

impl BitLength for Rate {
    fn bit_length(&self) -> u64 {
        self.value
    }
}

impl BitLength for OutputLength {
    fn bit_length(&self) -> u64 {
        self.value()
    }
}
