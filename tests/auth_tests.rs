use findora_backend::auth::{create_jwt, verify_jwt};

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
