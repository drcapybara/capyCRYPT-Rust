use crate::{
    curve::edwards::{
        order, EdCurvePoint,
        EdCurves::{self, E448},
        Generator,
    },
    sha3::{
        aux_functions::{
            byte_utils::{
                big_to_bytes, bytes_to_big, get_date_and_time_as_string, get_random_bytes,
                xor_bytes,
            },
            nist_800_185::{byte_pad, encode_string, right_encode},
        },
        sponge::{update, finalize},
    },
    Hashable, KeyEncryptable, KeyPair, Message, PwEncryptable, Signable, Signature,
};
use num_bigint::BigInt as Integer;
const SELECTED_CURVE: EdCurves = E448;

/*
============================================================
The main components of the cryptosystem are defined here
as trait implementations on specific types. The types and
their traits are defined in lib.rs. The arguments to all
operations mirror the notation from NIST FIPS 202 wherever
possible.

The Message type contains a data field. All operations are
performed IN PLACE. Future improvements to this library
will see computation moved off of the heap and batched.
============================================================
*/

/// # SHA3-Keccak
/// ref NIST FIPS 202.
/// ## Arguments:
/// * n: reference to message to be hashed.
/// * d: requested output length and security strength
/// ## Returns:
/// * SHA3-d message digest
fn shake(n: &mut Vec<u8>, d: u64) -> Vec<u8> {
    let bytes_to_pad = 136 - n.len() % 136; // SHA3-256 r = 1088 / 8 = 136
    if bytes_to_pad == 1 {
        //delim suffix
        n.extend_from_slice(&[0x86]);
    } else {
        //delim suffix
        n.extend_from_slice(&[0x06]);
    }
    finalize(&mut update(n, 2 * d), d, 1600 - (2 * d))
}

/// # Customizable SHAKE
/// Implements FIPS 202 Section 3. Returns: customizable and
/// domain-seperated length `L` SHA3XOF hash of input string.
/// ## Arguments:
/// * x: input message
/// * l: requested output length
/// * n: optional function name string
/// * s: option customization string
/// ## Returns:
/// * SHA3XOF hash of length `l` of input message `x`
pub fn cshake(x: &[u8], l: u64, n: &str, s: &str, d: u64) -> Vec<u8> {
    let mut encoded_n = encode_string(&n.as_bytes().to_vec());
    let encoded_s = encode_string(&s.as_bytes().to_vec());
    encoded_n.extend_from_slice(&encoded_s);

    let bytepad_w = match d {
        224 => 172,
        256 => 168,
        384 => 152,
        512 => 136,
        _ => panic!("Unsupported security strength. Must be 224, 384, 256, or 512"),
    };

    let mut out = byte_pad(&mut encoded_n, bytepad_w);
    out.extend_from_slice(x);
    out.push(0x04);

    if n.is_empty() && s.is_empty() {
        shake(&mut out, l);
    }

    finalize(&mut update(&mut out, d), l, 1600 - d)
}

/// # Keyed Message Authtentication
/// Generates keyed hash for given input as specified in NIST SP 800-185 section 4.
/// ## Arguments:
/// * k: key. SP 800 185 8.4.1 KMAC Key Length requires key length >= d
/// * x: byte-oriented message
/// * l: requested bit output length
/// * s: customization string
/// * d: the security parameter for the operation. NIST-standard values for d consist of the following:
/// - * d = 512; 256 bits of security
/// - * d = 256; 128 bits of security
///
/// ## Returns:
/// * kmac_xof of `x` under `k`
pub fn kmac_xof(k: &Vec<u8>, x: &[u8], l: u64, s: &str, d: u64) -> Vec<u8> {
    let mut encode_k = encode_string(k);
    let bytepad_w = match d {
        224 => 172,
        256 => 168,
        384 => 152,
        512 => 136,
        _ => panic!("Unsupported security strength. Must be 224, 384, 256, or 512"),
    };
    let mut bp = byte_pad(&mut encode_k, bytepad_w);

    // Extend bp with contents of x and right_encode(0)
    bp.extend_from_slice(x);
    bp.extend_from_slice(&right_encode(0)); // SP 800-185 4.3.1 KMAC with Arbitrary-Length Output

    cshake(&bp, l, "KMAC", s, d)
}

impl Hashable for Message {
    /// # Message Digest
    /// Computes SHA3-d hash of input. Does not consume input.
    /// Replaces `Message.digest` with result of operation.
    /// ## Arguments:
    /// * d: requested security strength in bits. Supported
    /// bitstrengths are 224, 256, 384, or 512.
    /// ## Usage:
    /// ```
    /// use capycrypt::{Hashable, Message};
    /// // Hash the empty string
    /// let mut data = Message::new(vec![]);
    /// // Obtained from OpenSSL
    /// let expected = "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a";
    /// // Compute a SHA3 digest with 256 bits of security
    /// data.compute_sha3_hash(256);
    /// assert!(hex::encode(data.digest.unwrap().to_vec()) == expected);
    /// ```
    fn compute_sha3_hash(&mut self, d: u64) {
        self.digest = match d {
            224 | 256 | 384 | 512 => Some(shake(&mut self.msg, d)),
            _ => panic!("Value must be either 224, 256, 384, or 512"),
        }
    }

    /// # Tagged Hash
    /// Computes an authentication tag `t` of a byte array `m` under passphrase `pw`.
    /// ## Replaces:
    /// * `Message.t` with keyed hash of plaintext.
    /// ## Arguments:
    /// * pw: symmetric encryption key, can be blank but shouldnt be
    /// * message: message to encrypt
    /// * s: domain seperation string
    /// * d: requested security strength in bits. Supported
    /// bitstrengths are 224, 256, 384, or 512.
    /// ## Usage:
    /// ```
    /// use capycrypt::{Hashable, Message};
    /// let mut pw = "test".as_bytes().to_vec();
    /// let mut data = Message::new(vec![]);
    /// let expected = "0f9b5dcd47dc08e08a173bbe9a57b1a65784e318cf93cccb7f1f79f186ee1caeff11b12f8ca3a39db82a63f4ca0b65836f5261ee64644ce5a88456d3d30efbed";
    /// data.compute_tagged_hash(&mut pw, &"", 512);
    /// assert!(hex::encode(data.digest.unwrap().to_vec()) == expected);
    /// ```
    fn compute_tagged_hash(&mut self, pw: &mut Vec<u8>, s: &str, d: u64) {
        self.digest = match d {
            224 | 256 | 384 | 512 => Some(kmac_xof(pw, &self.msg, d, s, d)),
            _ => panic!("Value must be either 224, 256, 384, or 512"),
        }
    }
}

impl PwEncryptable for Message {
    /// # Symmetric Encryption
    /// Encrypts a [`Message`] m symmetrically under passphrase pw.
    /// ## Replaces:
    /// * `Message.data` with result of encryption.
    /// * `Message.t` with keyed hash of plaintext.
    /// * `Message.sym_nonce` with z, as defined below.
    /// SECURITY NOTE: ciphertext length == plaintext length
    /// ## Algorithm:
    /// * z ← Random(512)
    /// * (ke || ka) ← kmac_xof(z || pw, “”, 1024, “S”)
    /// * c ← kmac_xof(ke, “”, |m|, “SKE”) ⊕ m
    /// * t ← kmac_xof(ka, m, 512, “SKA”)
    /// ## Arguments:
    /// * pw: symmetric encryption key, can be blank but shouldnt be
    /// * d: requested security strength in bits. Supported
    /// bitstrengths are 224, 256, 384, or 512.
    /// ## Usage:
    /// ```
    /// use capycrypt::{
    ///     Message,
    ///     PwEncryptable,
    ///     sha3::{aux_functions::{byte_utils::{get_random_bytes}}}
    /// };
    /// // Get a random password
    /// let pw = get_random_bytes(64);
    /// // Get 5mb random data
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Encrypt the data with 512 bits of security
    /// msg.pw_encrypt(&pw, 512);
    /// // Decrypt the data
    /// msg.pw_decrypt(&pw);
    /// // Verify operation success
    /// assert!(msg.op_result.unwrap());
    /// ```
    fn pw_encrypt(&mut self, pw: &[u8], d: u64) {
        self.d = Some(d);
        let z = get_random_bytes(512);
        let mut ke_ka = z.clone();
        ke_ka.append(&mut pw.to_owned());
        let ke_ka = kmac_xof(&ke_ka, &[], 1024, "S", d);
        let ke = &ke_ka[..64].to_vec();
        let ka = &ke_ka[64..].to_vec();
        self.digest = Some(kmac_xof(ka, &self.msg, 512, "SKA", d));
        let c = kmac_xof(ke, &[], (self.msg.len() * 8) as u64, "SKE", d);
        xor_bytes(&mut self.msg, &c);
        self.sym_nonce = Some(z);
    }

    /// # Symmetric Decryption
    /// Decrypts a [`Message`] (z, c, t) under passphrase pw.
    /// ## Assumes:
    /// * well-formed encryption
    /// * Some(Message.t)
    /// * Some(Message.z)
    /// ## Replaces:
    /// * `Message.data` with result of decryption.
    /// * `Message.op_result` with result of comparision of `Message.t` == keyed hash of decryption.
    /// ## Algorithm:
    /// * (ke || ka) ← kmac_xof(z || pw, “”, 1024, “S”)
    /// * m ← kmac_xof(ke, “”, |c|, “SKE”) ⊕ c
    /// * t’ ← kmac_xof(ka, m, 512, “SKA”)
    /// ## Arguments:
    /// * pw: decryption password, can be blank
    /// ## Usage:
    /// ```
    /// use capycrypt::{
    ///     Message,
    ///     PwEncryptable,
    ///     sha3::{aux_functions::{byte_utils::{get_random_bytes}}}
    /// };
    /// // Get a random password
    /// let pw = get_random_bytes(64);
    /// // Get 5mb random data
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Encrypt the data with 512 bits of security
    /// msg.pw_encrypt(&pw, 512);
    /// // Decrypt the data
    /// msg.pw_decrypt(&pw);
    /// // Verify operation success
    /// assert!(msg.op_result.unwrap());
    /// ```
    fn pw_decrypt(&mut self, pw: &[u8]) {
        let mut z_pw = self.sym_nonce.clone().unwrap();
        z_pw.append(&mut pw.to_owned());
        let ke_ka = kmac_xof(&z_pw, &[], 1024, "S", self.d.unwrap());
        let ke = &mut ke_ka[..64].to_vec();
        let ka = &mut ke_ka[64..].to_vec();
        let m = kmac_xof(ke, &[], (self.msg.len() * 8) as u64, "SKE", self.d.unwrap());
        xor_bytes(&mut self.msg, &m);
        let new_t = &kmac_xof(ka, &self.msg, 512, "SKA", self.d.unwrap());
        self.op_result = Some(self.digest.as_mut().unwrap() == new_t);
    }
}

impl KeyPair {
    /// # Asymmetric [`KeyPair`] Generation
    /// Generates a (Schnorr/ECDHIES) key pair from passphrase pw.
    ///
    /// ## Algorithm:
    /// * s ← kmac_xof(pw, “”, 512, “K”); s ← 4s
    /// * 𝑉 ← s*𝑮
    /// * key pair: (s, 𝑉)
    /// ## Arguments:
    /// * pw: password as bytes, can be blank but shouldnt be
    /// * owner: A label to indicate the owner of the key
    /// * curve: The selected Edwards curve
    /// ## Returns:
    /// * return  -> [`KeyPair`]: Key object containing owner, private key, public key x and y coordinates, and timestamp.
    /// verification key 𝑉 is hashed together with the message 𝑚
    /// and the nonce 𝑈: hash (𝑚, 𝑈, 𝑉) .
    /// ## Usage:
    /// ```  
    /// use capycrypt::{
    ///     curves::EdCurves::E448, KeyPair,
    ///     sha3::{aux_functions::{byte_utils::{get_random_bytes}}}
    /// };
    /// // Get a random password
    /// let pw = get_random_bytes(64);
    /// let key_pair = KeyPair::new(&pw, "test key".to_string(), E448, 512);
    /// ```
    pub fn new(pw: &Vec<u8>, owner: String, curve: EdCurves, d: u64) -> KeyPair {
        // Timing sidechannel on variable keysize is mitigated here due to mul 4 and modding by curve order,
        // ensuring top bits are always set.
        let s: Integer = (bytes_to_big(kmac_xof(pw, &[], 512, "K", d)) * 4) % order(SELECTED_CURVE);

        let pub_key = EdCurvePoint::generator(curve, false) * (s);

        KeyPair {
            owner,
            pub_key,
            priv_key: pw.to_vec(),
            date_created: get_date_and_time_as_string(),
            curve,
        }
    }
}

impl KeyEncryptable for Message {
    /// # Asymmetric Encryption
    /// Encrypts a [`Message`] in place under the (Schnorr/ECDHIES) public key 𝑉.
    /// Operates under Schnorr/ECDHIES principle in that shared symmetric key is
    /// exchanged with recipient. SECURITY NOTE: ciphertext length == plaintext length
    /// ## Replaces:
    /// * `Message.data` with result of encryption.
    /// * `Message.t` with keyed hash of plaintext.
    /// * `Message.asym_nonce` with z, as defined below.
    /// ## Algorithm:
    /// * k ← Random(512); k ← 4k
    /// * W ← kV; 𝑍 ← k*𝑮
    /// * (ke || ka) ← kmac_xof(W x , “”, 1024, “P”)
    /// * c ← kmac_xof(ke, “”, |m|, “PKE”) ⊕ m
    /// * t ← kmac_xof(ka, m, 512, “PKA”)
    /// ## Arguments:
    /// * pub_key: [`EdCurvePoint`] : X coordinate of public key 𝑉
    /// * d: Requested security strength in bits. Can only be 224, 256, 384, or 512.
    /// ## Usage:
    /// ```
    /// use capycrypt::{
    ///     KeyEncryptable,
    ///     KeyPair,
    ///     Message,
    ///     sha3::aux_functions::byte_utils::get_random_bytes,
    ///     curves::EdCurves::E448};
    /// // Get 5mb random data
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Generate the keypair
    /// let key_pair = KeyPair::new(&get_random_bytes(32), "test key".to_string(), E448, 512);
    /// // Encrypt with the public key
    /// msg.key_encrypt(&key_pair.pub_key, 512);
    /// ```
    fn key_encrypt(&mut self, pub_key: &EdCurvePoint, d: u64) {
        self.d = Some(d);
        let k: Integer = (bytes_to_big(get_random_bytes(64)) * 4) % order(pub_key.curve);
        let w = pub_key.clone() * k.clone();
        let z = EdCurvePoint::generator(pub_key.curve, false) * k;

        let ke_ka = kmac_xof(&big_to_bytes(w.x), &[], 1024, "PK", d);
        let ke = &mut ke_ka[..64].to_vec();
        let ka = &mut ke_ka[64..].to_vec();

        let t = kmac_xof(ka, &self.msg, 512, "PKA", d);
        let c = kmac_xof(ke, &[], (self.msg.len() * 8) as u64, "PKE", d);
        xor_bytes(&mut self.msg, &c);

        self.digest = Some(t);
        self.asym_nonce = Some(z);
    }

    /// # Asymmetric Decryption
    /// Decrypts a [`Message`] in place under private key.
    /// Operates under Schnorr/ECDHIES principle in that shared symmetric key is
    /// derived from 𝑍.
    ///
    /// ## Assumes:
    /// * well-formed encryption
    /// * Some(Message.t)
    /// * Some(Message.z)
    ///
    /// ## Replaces:
    /// * `Message.data` with result of decryption.
    /// * `Message.op_result` with result of comparision of `Message.t` == keyed hash of decryption.
    ///
    /// ## Algorithm:
    /// * s ← KMACXOF256(pw, “”, 512, “K”); s ← 4s
    /// * W ← sZ
    /// * (ke || ka) ← KMACXOF256(W x , “”, 1024, “P”)
    /// * m ← KMACXOF256(ke, “”, |c|, “PKE”) ⊕ c
    /// * t’ ← KMACXOF256(ka, m, 512, “PKA”)
    ///
    /// ## Arguments:
    /// * pw: password used to generate ```CurvePoint``` encryption key.
    /// * d: encryption security strength in bits. Can only be 224, 256, 384, or 512.
    ///
    /// ## Usage:
    /// ```
    /// use capycrypt::{
    ///     KeyEncryptable,
    ///     KeyPair,
    ///     Message,
    ///     sha3::aux_functions::byte_utils::get_random_bytes,
    ///     curves::EdCurves::E448};
    ///
    /// // Get 5mb random data
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Create a new private/public keypair
    /// let key_pair = KeyPair::new(&get_random_bytes(32), "test key".to_string(), E448, 512);
    ///
    /// // Encrypt the message
    /// msg.key_encrypt(&key_pair.pub_key, 512);
    /// //Decrypt the message
    /// msg.key_decrypt(&key_pair.priv_key);
    /// // Verify
    /// assert!(msg.op_result.unwrap());
    /// ```
    fn key_decrypt(&mut self, pw: &[u8]) {
        let z = self.asym_nonce.clone().unwrap();
        let s: Integer = (bytes_to_big(kmac_xof(&pw.to_owned(), &[], 512, "K", self.d.unwrap()))
            * 4)
            % z.clone().n;
        let w = z * s;

        let ke_ka = kmac_xof(&big_to_bytes(w.x), &[], 1024, "PK", self.d.unwrap());
        let ke = &mut ke_ka[..64].to_vec();
        let ka = &mut ke_ka[64..].to_vec();

        let m = Box::new(kmac_xof(
            ke,
            &[],
            (self.msg.len() * 8) as u64,
            "PKE",
            self.d.unwrap(),
        ));
        xor_bytes(&mut self.msg, &m);
        let t_p = kmac_xof(ka, &self.msg, 512, "PKA", self.d.unwrap());
        self.op_result = Some(t_p == self.digest.as_deref().unwrap());
    }
}

impl Signable for Message {
    /// # Schnorr Signatures
    /// Signs a [`Message`] under passphrase pw.
    ///
    /// ## Algorithm:
    /// * `s` ← kmac_xof(pw, “”, 512, “K”); s ← 4s
    /// * `k` ← kmac_xof(s, m, 512, “N”); k ← 4k
    /// * `𝑈` ← k*𝑮;
    /// * `ℎ` ← kmac_xof(𝑈ₓ , m, 512, “T”); 𝑍 ← (𝑘 – ℎ𝑠) mod r
    ///
    /// ## Arguments:
    /// * key: reference to KeyPair.
    /// * d: encryption security strength in bits. Can only be 224, 256, 384, or 512.
    ///
    /// ## Assumes:
    /// * Some(key.priv_key)
    ///
    /// ## Usage
    /// ```
    /// use capycrypt::{
    ///     Signable,
    ///     KeyPair,
    ///     Message,
    ///     sha3::aux_functions::byte_utils::get_random_bytes,
    ///     curves::EdCurves::E448};
    /// // Get random 5mb
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Get a random password
    /// let pw = get_random_bytes(64);
    /// // Generate a signing keypair
    /// let key_pair = KeyPair::new(&pw, "test key".to_string(), E448, 512);
    /// // Sign with 512 bits of security
    /// msg.sign(&key_pair, 512);
    /// ```
    fn sign(&mut self, key: &KeyPair, d: u64) {
        self.d = Some(d);
        let s: Integer = bytes_to_big(kmac_xof(&key.priv_key, &[], 512, "K", d)) * 4;
        let s_bytes = big_to_bytes(s.clone());

        let k: Integer = bytes_to_big(kmac_xof(&s_bytes, &self.msg, 512, "N", d)) * 4;

        let u = EdCurvePoint::generator(SELECTED_CURVE, false) * k.clone();
        let ux_bytes = big_to_bytes(u.x);
        let h = kmac_xof(&ux_bytes, &self.msg, 512, "T", d);
        let h_big = bytes_to_big(h.clone());
        //(a % b + b) % b
        let z = ((k - (h_big * s)) % u.r.clone() + u.r.clone()) % u.r;
        self.sig = Some(Signature { h, z })
    }
    /// # Signature Verification
    /// Verifies a [`Signature`] (h, 𝑍) for a byte array m under the (Schnorr/
    /// ECDHIES) public key 𝑉.
    /// ## Algorithm:
    /// * 𝑈 ← 𝑍*𝑮 + h𝑉
    /// ## Arguments:
    /// * sig: &[`Signature`]: Pointer to a signature object (h, 𝑍)
    /// * pubKey: CurvePoint key 𝑉 used to sign message m
    /// * message: Vec<u8> of message to verify
    /// ## Assumes:
    /// * Some(key.pub_key)
    /// * Some([`Message`].sig)
    /// ## Usage
    /// ```
    /// use capycrypt::{
    ///     Signable,
    ///     KeyPair,
    ///     Message,
    ///     sha3::aux_functions::byte_utils::get_random_bytes,
    ///     curves::EdCurves::E448};
    /// // Get random 5mb
    /// let mut msg = Message::new(get_random_bytes(5242880));
    /// // Get a random password
    /// let pw = get_random_bytes(64);
    /// // Generate a signing keypair
    /// let key_pair = KeyPair::new(&pw, "test key".to_string(), E448, 512);
    /// // Sign with 512 bits of security
    /// msg.sign(&key_pair, 512);
    /// // Verify
    /// msg.verify(&key_pair.pub_key);
    /// assert!(msg.op_result.unwrap());
    /// ```
    fn verify(&mut self, pub_key: &EdCurvePoint) {
        let mut u = EdCurvePoint::generator(pub_key.curve, false) * self.sig.clone().unwrap().z;
        let hv = pub_key.clone() * bytes_to_big(self.sig.clone().unwrap().h);
        u = u + &hv;
        let h_p = kmac_xof(&big_to_bytes(u.x), &self.msg, 512, "T", self.d.unwrap());
        self.op_result = Some(h_p == self.sig.clone().unwrap().h)
    }
}
