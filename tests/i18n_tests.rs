use findora_backend::i18n::{
    msg_account_locked_until_tomorrow, msg_default_wallet_name, msg_login_success,
    msg_register_success, msg_wrong_password_with_remaining, Language,
};

#[test]
fn test_language_detection() {
    assert_eq!(Language::from_header_value("en-US,en;q=0.9"), Language::En);
    assert_eq!(Language::from_header_value("en"), Language::En);
    assert_eq!(Language::from_header_value("vi-VN,vi;q=0.9"), Language::Vi);
    assert_eq!(Language::from_header_value("fr-FR"), Language::Vi); // fallback mặc định là Tiếng Việt
}

#[test]
fn test_bilingual_messages() {
    assert_eq!(msg_login_success(Language::Vi), "Đăng nhập thành công");
    assert_eq!(msg_login_success(Language::En), "Login successful");
    assert_eq!(msg_register_success(Language::Vi), "Đăng ký tài khoản thành công!");
    assert_eq!(msg_register_success(Language::En), "Account registered successfully!");
    assert_eq!(msg_default_wallet_name(Language::Vi), "Ví tiền mặt");
    assert_eq!(msg_default_wallet_name(Language::En), "Cash Wallet");

    assert!(msg_account_locked_until_tomorrow(Language::Vi).contains("tạm khóa"));
    assert!(msg_account_locked_until_tomorrow(Language::En).contains("temporarily locked"));

    let wrong_vi = msg_wrong_password_with_remaining(Language::Vi, 7);
    assert!(wrong_vi.contains("còn 7 lần thử"));

    let wrong_en = msg_wrong_password_with_remaining(Language::En, 5);
    assert!(wrong_en.contains("5 attempt(s) remaining"));
}
