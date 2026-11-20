use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::ACCEPT_LANGUAGE, request::Parts},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Vi,
    En,
}

impl Language {
    pub fn from_header_value(header_val: &str) -> Self {
        let lower = header_val.to_lowercase();
        // Kiểm tra nếu client gửi en, en-US, en-GB...
        if lower.starts_with("en") || lower.contains(",en") {
            Language::En
        } else {
            Language::Vi
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::Vi => "vi",
            Language::En => "en",
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Language
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let lang = parts
            .headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|h| h.to_str().ok())
            .map(Language::from_header_value)
            .unwrap_or(Language::Vi);

        Ok(lang)
    }
}

// =========================================================================
// Tập hợp các thông điệp phản hồi Anh - Việt (Localized Response Messages)
// =========================================================================

pub fn msg_register_success(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Đăng ký tài khoản thành công!",
        Language::En => "Account registered successfully!",
    }
}

pub fn msg_login_success(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Đăng nhập thành công",
        Language::En => "Login successful",
    }
}

pub fn msg_logout_success(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Đăng xuất tài khoản thành công",
        Language::En => "Logged out successfully",
    }
}

pub fn msg_invalid_credentials(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Email hoặc mật khẩu không chính xác",
        Language::En => "Invalid email or password",
    }
}

pub fn msg_email_exists(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Email này đã được đăng ký tài khoản Findora",
        Language::En => "This email is already registered with Findora",
    }
}

pub fn msg_invalid_email(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Email không đúng định dạng",
        Language::En => "Invalid email format",
    }
}

pub fn msg_empty_name(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Họ và tên không được để trống",
        Language::En => "Full name cannot be empty",
    }
}

pub fn msg_password_min_length(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Mật khẩu phải chứa ít nhất 8 ký tự",
        Language::En => "Password must contain at least 8 characters",
    }
}

pub fn msg_password_max_length(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Mật khẩu không được vượt quá 72 ký tự",
        Language::En => "Password cannot exceed 72 characters",
    }
}

pub fn msg_password_need_digit(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Mật khẩu phải chứa ít nhất một chữ số (0-9)",
        Language::En => "Password must contain at least one digit (0-9)",
    }
}

pub fn msg_password_need_special(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Mật khẩu phải chứa ít nhất một ký tự đặc biệt (!@#$%^&*...)",
        Language::En => "Password must contain at least one special character (!@#$%^&*...)",
    }
}

pub fn msg_user_not_found(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Người dùng không tồn tại",
        Language::En => "User not found",
    }
}

pub fn msg_default_wallet_name(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Ví tiền mặt",
        Language::En => "Cash Wallet",
    }
}

pub fn msg_avatar_upload_success(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Cập nhật ảnh đại diện thành công",
        Language::En => "Avatar updated successfully",
    }
}

pub fn msg_empty_wallet_name(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Tên ví không được để trống",
        Language::En => "Wallet name cannot be empty",
    }
}

pub fn msg_invalid_amount(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Số tiền phải lớn hơn 0",
        Language::En => "Amount must be greater than 0",
    }
}

pub fn msg_invalid_tx_type(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Loại giao dịch không hợp lệ ('EXPENSE', 'INCOME', 'TRANSFER')",
        Language::En => "Invalid transaction type ('EXPENSE', 'INCOME', 'TRANSFER')",
    }
}

pub fn msg_wallet_not_found(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Ví không tồn tại hoặc không thuộc quyền sở hữu của bạn",
        Language::En => "Wallet does not exist or does not belong to you",
    }
}

pub fn msg_no_wallet_available(lang: Language) -> &'static str {
    match lang {
        Language::Vi => "Bạn chưa có ví tài chính nào. Hãy tạo ví trước.",
        Language::En => "You do not have any wallets. Please create a wallet first.",
    }
}

