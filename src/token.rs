use ct_codecs::{Base64UrlSafeNoPadding, Decoder, Encoder};
use serde::{de::DeserializeOwned, Serialize};

use crate::claims::*;
use crate::common::*;
use crate::error::*;
use crate::jwt_header::*;

pub const MAX_HEADER_LENGTH: usize = 4096;

/// Utilities to get information about a JWT token
pub struct Token;

/// JWT token information useful before signature/tag verification
pub struct TokenMetadata {
    jwt_header: JWTHeader,
}

impl TokenMetadata {
    /// The JWT algorithm for this token
    pub fn algorithm(&self) -> &str {
        &self.jwt_header.algorithm
    }

    /// The content type for this token
    pub fn content_type(&self) -> Option<&str> {
        self.jwt_header.content_type.as_deref()
    }

    /// The key set URL for this token
    pub fn key_set_url(&self) -> Option<&str> {
        self.jwt_header.key_set_url.as_deref()
    }

    /// The public key for this token
    pub fn public_key(&self) -> Option<&str> {
        self.jwt_header.public_key.as_deref()
    }

    /// The key, or public key identifier for this token
    pub fn key_id(&self) -> Option<&str> {
        self.jwt_header.key_id.as_deref()
    }

    /// The certificate URL for this token
    pub fn certificate_url(&self) -> Option<&str> {
        self.jwt_header.certificate_url.as_deref()
    }

    /// The certificate chain for this token
    pub fn certificate_chain(&self) -> Option<&str> {
        self.jwt_header.certificate_chain.as_deref()
    }

    /// The SHA-1 certificate fingerprint
    pub fn certificate_sha1_thumbprint(&self) -> Option<&str> {
        self.jwt_header.certificate_sha1_thumbprint.as_deref()
    }

    /// The SHA-256 certificate fingerprint
    pub fn certificate_sha256_thumbprint(&self) -> Option<&str> {
        self.jwt_header.certificate_sha256_thumbprint.as_deref()
    }

    /// The signature type for this token
    pub fn signature_type(&self) -> Option<&str> {
        self.jwt_header.signature_type.as_deref()
    }

    /// The set of critical properties for this token
    pub fn critical(&self) -> Option<&str> {
        self.jwt_header.critical.as_deref()
    }
}

impl Token {
    pub(crate) fn build<AuthenticationOrSignatureFn, CustomClaims: Serialize + DeserializeOwned>(
        jwt_header: &JWTHeader,
        claims: JWTClaims<CustomClaims>,
        authentication_or_signature_fn: AuthenticationOrSignatureFn,
    ) -> Result<String, Error>
    where
        AuthenticationOrSignatureFn: FnOnce(&str) -> Result<Vec<u8>, Error>,
    {
        let jwt_header_json = serde_json::to_string(&jwt_header)?;
        let claims_json = serde_json::to_string(&claims)?;
        let authenticated = format!(
            "{}.{}",
            Base64UrlSafeNoPadding::encode_to_string(jwt_header_json)?,
            Base64UrlSafeNoPadding::encode_to_string(claims_json)?
        );
        let authentication_tag_or_signature = authentication_or_signature_fn(&authenticated)?;
        let mut token = authenticated;
        token.push('.');
        token.push_str(&Base64UrlSafeNoPadding::encode_to_string(
            &authentication_tag_or_signature,
        )?);
        Ok(token)
    }

    pub(crate) fn verify<AuthenticationOrSignatureFn, CustomClaims: Serialize + DeserializeOwned>(
        jwt_alg_name: &'static str,
        token: &str,
        options: Option<VerificationOptions>,
        authentication_or_signature_fn: AuthenticationOrSignatureFn,
    ) -> Result<JWTClaims<CustomClaims>, Error>
    where
        AuthenticationOrSignatureFn: FnOnce(&str, &[u8]) -> Result<(), Error>,
    {
        let options = options.unwrap_or_default();
        let mut parts = token.split('.');
        let jwt_header_b64 = parts.next().ok_or(JWTError::CompactEncodingError)?;
        ensure!(
            jwt_header_b64.len() <= MAX_HEADER_LENGTH,
            JWTError::HeaderTooLarge
        );
        let claims_b64 = parts.next().unwrap();
        let authentication_tag_b64 = parts.next().ok_or(JWTError::CompactEncodingError)?;
        ensure!(parts.next().is_none(), JWTError::CompactEncodingError);
        let jwt_header: JWTHeader = serde_json::from_slice(
            &Base64UrlSafeNoPadding::decode_to_vec(jwt_header_b64, None).unwrap(),
        )?;
        ensure!(
            jwt_header.algorithm == jwt_alg_name,
            JWTError::AlgorithmMismatch
        );
        if let Some(required_key_id) = &options.required_key_id {
            if let Some(key_id) = &jwt_header.key_id {
                ensure!(key_id == required_key_id, JWTError::KeyIdentifierMismatch);
            } else {
                bail!(JWTError::MissingJWTKeyIdentifier)
            }
        }
        let authentication_tag =
            Base64UrlSafeNoPadding::decode_to_vec(&authentication_tag_b64, None)?;
        let authenticated = &token[..jwt_header_b64.len() + 1 + claims_b64.len()];
        authentication_or_signature_fn(authenticated, &authentication_tag)?;
        let claims: JWTClaims<CustomClaims> =
            serde_json::from_slice(&Base64UrlSafeNoPadding::decode_to_vec(&claims_b64, None)?)?;
        claims.validate(&options)?;
        Ok(claims)
    }

    /// Decode token information that can be usedful prior to signature/tag verification
    pub fn decode_metadata(token: &str) -> Result<TokenMetadata, Error> {
        let mut parts = token.split('.');
        let jwt_header_b64 = parts.next().ok_or(JWTError::CompactEncodingError)?;
        ensure!(
            jwt_header_b64.len() <= MAX_HEADER_LENGTH,
            JWTError::HeaderTooLarge
        );
        let jwt_header: JWTHeader = serde_json::from_slice(
            &Base64UrlSafeNoPadding::decode_to_vec(jwt_header_b64, None).unwrap(),
        )?;
        Ok(TokenMetadata { jwt_header })
    }
}
