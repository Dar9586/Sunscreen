use rlp::encode_list;
use seal_fhe::SecurityLevel;
pub use semver::Version;
use serde::{Deserialize, Serialize};
use sunscreen_compiler_common::Type;
use sunscreen_fhe_program::{FheProgram, SchemeType};

use crate::{Error, Result};

/**
 * Indicates the type signatures of an Fhe Program. Serves as a piece of the [`FheProgramMetadata`].
 *
 * # Remarks
 * This type is serializable and FHE program implementors can give this object
 * to consumers without revealing this FHE program's implementation. This allows
 * users to encrypt their data in a verifiable manner.
 */
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSignature {
    /**
     * The type of each argument in the FHE program.
     *
     * # Remarks
     * The ith argument to the FHE program occupies the ith argument of the vector.
     * The length of this vector equals the number of arguments to the FHE program.
     */
    pub arguments: Vec<Type>,

    /**
     * The type of the single return value of the FHE program if the return type is
     * not a type. If the return type of the FHE program is a tuple, then this contains
     * each type in the tuple.
     *
     * # Remarks
     * The ith argument to the FHE program occupies the ith argument of the vector.
     * The length of this vector equals the number of arguments to the FHE program.
     */
    pub returns: Vec<Type>,

    /**
     * The number of ciphertexts that compose the nth return value.
     */
    pub num_ciphertexts: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "debugger", serde(tag = "type"))]
/**
 * A key type required for an Fhe Program to function correctly.
 */
pub enum RequiredKeys {
    /**
     * The FHE program performs Batched shifts and requires Galois keys.
     */
    Galois,
    /**
     * The FHE program performs relinearizations and requires relinearization keys.
     */
    Relin,

    /**
     * The FHE program performs an operation that requires the public encryption key.
     */
    PublicKey,
}

#[derive(Debug, Clone, Serialize, Hash, Deserialize, PartialEq, Eq)]
/**
 * The parameter set required for a given FHE program to run efficiently and correctly.
 */
pub struct Params {
    /**
     * The lattice dimension. For CKKS and BFV, this is the degree of the ciphertext polynomial.
     */
    pub lattice_dimension: u64,

    /**
     * The modulii for each modulo switch level for BFV and CKKS.
     */
    pub coeff_modulus: Vec<u64>,

    /**
     * The plaintext modulus.
     */
    pub plain_modulus: u64,

    /**
     * The scheme type.
     */
    pub scheme_type: SchemeType,

    /**
     * The securtiy level required.
     */
    pub security_level: SecurityLevel,
}

impl Params {
    /**
     * Serialize the params to a byte array.
     */
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend_from_slice(&self.lattice_dimension.to_be_bytes());
        bytes.extend_from_slice(&self.plain_modulus.to_be_bytes());

        let scheme_type: u8 = self.scheme_type.into();
        bytes.push(scheme_type);

        let security_level: i32 = self.security_level.into();
        bytes.extend_from_slice(&security_level.to_be_bytes());
        bytes.extend(encode_list(&self.coeff_modulus));

        bytes
    }

    /**
     * Attempt to read params from a byte array.
     */
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        let (lattice_dimension, rest) = Self::read_u64(bytes)?;
        let (plain_modulus, rest) = Self::read_u64(rest)?;

        let (scheme_type, rest) = Self::read_u8(rest)?;
        let scheme_type: SchemeType = scheme_type.try_into()?;

        let (security_level, rest) = Self::read_i32(rest)?;
        let security_level: SecurityLevel = security_level.try_into()?;

        let coeff_modulus: Vec<u64> = rlp::decode_list(rest);

        Ok(Self {
            lattice_dimension,
            plain_modulus,
            scheme_type,
            security_level,
            coeff_modulus,
        })
    }

    fn read_u64(bytes: &[u8]) -> Result<(u64, &[u8])> {
        let (int_bytes, rest) = bytes.split_at(std::mem::size_of::<u64>());
        let val = u64::from_be_bytes(
            int_bytes
                .try_into()
                .map_err(|_| Error::ParamDeserializationError)?,
        );

        Ok((val, rest))
    }

    fn read_i32(bytes: &[u8]) -> Result<(i32, &[u8])> {
        let (int_bytes, rest) = bytes.split_at(std::mem::size_of::<i32>());
        let val = i32::from_be_bytes(
            int_bytes
                .try_into()
                .map_err(|_| Error::ParamDeserializationError)?,
        );

        Ok((val, rest))
    }

    fn read_u8(bytes: &[u8]) -> Result<(u8, &[u8])> {
        let (int_bytes, rest) = bytes.split_at(std::mem::size_of::<u8>());
        let val = u8::from_be_bytes(
            int_bytes
                .try_into()
                .map_err(|_| Error::ParamDeserializationError)?,
        );

        Ok((val, rest))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/**
 * A serializable list of requirements for an Fhe Program.
 */
pub struct FheProgramMetadata {
    /**
     * The FHE scheme parameters required for encrypting data for use in the FHE program.
     */
    pub params: Params,

    /**
     * The call signature (arguments and returns) of the FHE program.
     */
    pub signature: CallSignature,

    /**
     * The set of keys required to run the FHE program.
     */
    pub required_keys: Vec<RequiredKeys>,

    /**
     * The name of the FHE program.
     */
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
/**
 * An FHE program with its associated metadata.
 */
pub struct CompiledFheProgram {
    /**
     * The underlying FHE FHE program.
     */
    pub fhe_program_fn: FheProgram,

    /**
     * Information about the FHE program, including its call signature and the scheme
     * parameters needed by a [`Runtime`](crate::Runtime) to encrypt/decrypt its inputs/outputs.
     */
    pub metadata: FheProgramMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_roundtrip_params() {
        let params = Params {
            lattice_dimension: 4096,
            plain_modulus: 64,
            coeff_modulus: vec![1, 2, 3, 4],
            security_level: SecurityLevel::TC192,
            scheme_type: SchemeType::Bfv,
        };

        let params_2 = Params::try_from_bytes(&params.to_bytes()).unwrap();

        assert_eq!(params, params_2);
    }

    #[test]
    fn can_serialize_deserialize_typename() {
        let typename = Type {
            name: "foo::Bar".to_owned(),
            version: Version::new(42, 24, 6),
            is_encrypted: false,
        };

        let serialized = serde_json::to_string(&typename).unwrap();
        let deserialized: Type = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.name, typename.name);
        assert_eq!(deserialized.version, typename.version);
    }
}
