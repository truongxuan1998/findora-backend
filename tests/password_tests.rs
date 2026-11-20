use findora_backend::errors::AppError;
use findora_backend::i18n::Language;
use findora_backend::routes::auth::validate_password;

#[test]
fn test_password_valid() {
    assert!(validate_password("Password123!", Language::Vi).is_ok());
    assert!(validate_password("Secure@2026", Language::En).is_ok());
    assert!(validate_password("Abc#1234", Language::Vi).is_ok());
}

#[test]
fn test_password_too_short() {
    let res = validate_password("Abc1!", Language::Vi);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), AppError::BadRequest(msg) if msg.contains("ít nhất 8 ký tự")));

    let res_en = validate_password("Abc1!", Language::En);
    assert!(res_en.is_err());
    assert!(matches!(res_en.unwrap_err(), AppError::BadRequest(msg) if msg.contains("at least 8 characters")));
}

#[test]
fn test_password_no_digit() {
    let res = validate_password("Password!@#", Language::Vi);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), AppError::BadRequest(msg) if msg.contains("chữ số")));

    let res_en = validate_password("Password!@#", Language::En);
    assert!(res_en.is_err());
    assert!(matches!(res_en.unwrap_err(), AppError::BadRequest(msg) if msg.contains("digit")));
}

#[test]
fn test_password_no_special() {
    let res = validate_password("Password123", Language::Vi);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), AppError::BadRequest(msg) if msg.contains("ký tự đặc biệt")));

    let res_en = validate_password("Password123", Language::En);
    assert!(res_en.is_err());
    assert!(matches!(res_en.unwrap_err(), AppError::BadRequest(msg) if msg.contains("special character")));
}

#[test]
fn test_password_too_long() {
    let long_pw = "A1!".repeat(25); // 75 chars
    let res = validate_password(&long_pw, Language::Vi);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), AppError::BadRequest(msg) if msg.contains("không được vượt quá 72 ký tự")));
}
