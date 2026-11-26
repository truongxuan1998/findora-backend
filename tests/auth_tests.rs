use findora_backend::auth::{
    check_token_version, create_jwt, create_versioned_jwt, verify_jwt, TOKEN_VALID_DAYS,
};
use jsonwebtoken::{encode, EncodingKey, Header};

const SECRET: &str = "test_super_secret_jwt_key_123456789";

#[test]
fn test_versioned_jwt_roundtrip_keeps_version() {
    let token = create_versioned_jwt("usr_v", "v@findora.app", 7, SECRET, TOKEN_VALID_DAYS).unwrap();
    let claims = verify_jwt(&token, SECRET).unwrap();
    assert_eq!(claims.ver, 7);
    assert_eq!(claims.sub, "usr_v");
}

#[test]
fn test_create_jwt_defaults_to_version_zero() {
    let token = create_jwt("usr_0", "zero@findora.app", SECRET, 1).unwrap();
    assert_eq!(verify_jwt(&token, SECRET).unwrap().ver, 0);
}

#[test]
fn test_legacy_token_without_ver_claim_is_version_zero() {
    // Token cấp trước khi có claim `ver` vẫn phải verify được và được coi là version 0
    let exp = (chrono::Utc::now() + chrono::Duration::days(1)).timestamp() as usize;
    let legacy = serde_json::json!({
        "sub": "usr_legacy",
        "email": "legacy@findora.app",
        "exp": exp,
        "iat": exp - 86_400,
    });
    let token = encode(&Header::default(), &legacy, &EncodingKey::from_secret(SECRET.as_bytes())).unwrap();

    let claims = verify_jwt(&token, SECRET).expect("legacy token must still verify");
    assert_eq!(claims.ver, 0);
    assert!(check_token_version(&claims, Some(0)).is_ok());
}

#[test]
fn test_check_token_version_rejects_revoked_or_deleted_user() {
    let token = create_versioned_jwt("usr_r", "r@findora.app", 1, SECRET, 1).unwrap();
    let claims = verify_jwt(&token, SECRET).unwrap();

    assert!(check_token_version(&claims, Some(1)).is_ok());
    // Version trong DB đã tăng (đặt lại mật khẩu / đăng xuất mọi thiết bị)
    assert!(check_token_version(&claims, Some(2)).is_err());
    // User đã bị xoá
    assert!(check_token_version(&claims, None).is_err());
}

#[test]
fn test_jwt_create_and_verify() {
    let secret = "test_super_secret_jwt_key_123456789";
    let user_id = "usr_1234567890abcdef";
    let email = "test@findora.app";

    let token = create_jwt(user_id, email, secret, 1).expect("Failed to create JWT");
    assert!(!token.is_empty());

    let claims = verify_jwt(&token, secret).expect("Failed to verify JWT");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.email, email);
}

#[test]
fn test_jwt_invalid_secret() {
    let secret = "test_super_secret_jwt_key_123456789";
    let wrong_secret = "wrong_secret_key_987654321";
    let token = create_jwt("usr_1", "test@findora.app", secret, 1).unwrap();

    let result = verify_jwt(&token, wrong_secret);
    assert!(result.is_err());
}
