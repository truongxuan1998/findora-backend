use crate::i18n::Language;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone, Debug)]
pub struct EmailService {
    client: Client,
    resend_api_key: Option<String>,
    resend_from: String,
    brevo_api_key: Option<String>,
    brevo_sender_email: String,
    brevo_sender_name: String,
}

#[derive(Serialize)]
struct ResendSendEmailRequest<'a> {
    from: &'a str,
    to: Vec<&'a str>,
    subject: &'a str,
    html: String,
}

#[derive(Serialize)]
struct BrevoRecipient<'a> {
    email: &'a str,
}

#[derive(Serialize)]
struct BrevoSender<'a> {
    name: &'a str,
    email: &'a str,
}

#[derive(Serialize)]
struct BrevoSendEmailRequest<'a> {
    sender: BrevoSender<'a>,
    to: Vec<BrevoRecipient<'a>>,
    subject: &'a str,
    #[serde(rename = "htmlContent")]
    html_content: String,
}

impl EmailService {
    pub fn new(
        resend_api_key: Option<String>,
        resend_from: String,
        brevo_api_key: Option<String>,
        brevo_sender_email: String,
        brevo_sender_name: String,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            resend_api_key,
            resend_from,
            brevo_api_key,
            brevo_sender_email,
            brevo_sender_name,
        }
    }

    pub fn disabled() -> Self {
        Self::new(
            None,
            "Findora Security <onboarding@resend.dev>".to_string(),
            None,
            "no-reply@findora.app".to_string(),
            "Findora Security".to_string(),
        )
    }

    /// Gửi email chứa mã OTP xác thực 6 số (Ưu tiên Resend, tự động fallback sang Brevo nếu lỗi)
    pub async fn send_reset_password_otp(
        &self,
        recipient_email: &str,
        code: &str,
        lang: Language,
    ) -> Result<(), String> {
        let subject = match lang {
            Language::Vi => "Mã xác thực đặt lại mật khẩu - Findora",
            Language::En => "Password Reset Verification Code - Findora",
        };

        let html_content = generate_otp_email_html(code, lang);

        // 1. Thử gửi qua Resend nếu có cấu hình RESEND_API_KEY
        if let Some(resend_key) = &self.resend_api_key {
            if !resend_key.trim().is_empty() {
                match self.send_via_resend(resend_key.trim(), recipient_email, subject, html_content.clone()).await {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        warn!(
                            recipient = %recipient_email,
                            error = %e,
                            "⚠️ Gửi email qua Resend không thành công, kiểm tra kênh fallback Brevo..."
                        );
                    }
                }
            }
        }

        // 2. Fallback hoặc gửi qua Brevo nếu có cấu hình BREVO_API_KEY
        if let Some(brevo_key) = &self.brevo_api_key {
            if !brevo_key.trim().is_empty() {
                return self.send_via_brevo(brevo_key.trim(), recipient_email, subject, html_content).await;
            }
        }

        // Nếu không có API key nào hoặc cả hai đều không gửi được
        if self.resend_api_key.is_none() && self.brevo_api_key.is_none() {
            warn!(
                recipient = %recipient_email,
                "⚠️ Chưa cấu hình RESEND_API_KEY hoặc BREVO_API_KEY. Bỏ qua bước gửi email thực tế (mã OTP vẫn được lưu bảo mật trong DB)."
            );
            Ok(())
        } else {
            Err("Không thể gửi email OTP qua các nhà cung cấp đã cấu hình".to_string())
        }
    }

    async fn send_via_resend(
        &self,
        api_key: &str,
        recipient_email: &str,
        subject: &str,
        html_content: String,
    ) -> Result<(), String> {
        let payload = ResendSendEmailRequest {
            from: &self.resend_from,
            to: vec![recipient_email],
            subject,
            html: html_content,
        };

        let res = self
            .client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối tới máy chủ Resend: {}", e))?;

        let status = res.status();
        let body = res
            .text()
            .await
            .unwrap_or_else(|_| "Không thể đọc nội dung phản hồi".to_string());

        if status.is_success() {
            info!(
                recipient = %recipient_email,
                "✅ Đã gửi email mã xác thực OTP 6 số qua Resend thành công! Response: {}",
                body
            );
            Ok(())
        } else {
            error!(
                status = %status,
                body = %body,
                recipient = %recipient_email,
                "❌ Gửi email qua Resend thất bại"
            );
            Err(format!("Resend API lỗi (HTTP {}): {}", status, body))
        }
    }

    async fn send_via_brevo(
        &self,
        api_key: &str,
        recipient_email: &str,
        subject: &str,
        html_content: String,
    ) -> Result<(), String> {
        let payload = BrevoSendEmailRequest {
            sender: BrevoSender {
                name: &self.brevo_sender_name,
                email: &self.brevo_sender_email,
            },
            to: vec![BrevoRecipient {
                email: recipient_email,
            }],
            subject,
            html_content,
        };

        let res = self
            .client
            .post("https://api.brevo.com/v3/smtp/email")
            .header("api-key", api_key)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối tới máy chủ Brevo: {}", e))?;

        let status = res.status();
        let body = res
            .text()
            .await
            .unwrap_or_else(|_| "Không thể đọc nội dung phản hồi".to_string());

        if status.is_success() {
            info!(
                recipient = %recipient_email,
                "✅ Đã gửi email mã xác thực OTP 6 số qua Brevo thành công!"
            );
            Ok(())
        } else {
            error!(
                status = %status,
                body = %body,
                recipient = %recipient_email,
                "❌ Gửi email qua Brevo thất bại"
            );
            Err(format!("Brevo API lỗi (HTTP {}): {}", status, body))
        }
    }
}

fn generate_otp_email_html(code: &str, lang: Language) -> String {
    match lang {
        Language::Vi => format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Mã xác thực đặt lại mật khẩu</title>
</head>
<body style="margin: 0; padding: 0; background-color: #f4f5f7; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;">
    <table width="100%" border="0" cellspacing="0" cellpadding="0" style="background-color: #f4f5f7; padding: 30px 15px;">
        <tr>
            <td align="center">
                <table width="100%" border="0" cellspacing="0" cellpadding="0" style="max-width: 520px; background-color: #ffffff; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 12px rgba(0,0,0,0.06);">
                    <!-- Header -->
                    <tr>
                        <td align="center" style="background: linear-gradient(135deg, #0FA37F 0%, #177264 100%); padding: 32px 20px;">
                            <h1 style="margin: 0; color: #ffffff; font-size: 26px; font-weight: 800; letter-spacing: 2px;">FINDORA</h1>
                            <p style="margin: 6px 0 0; color: rgba(255,255,255,0.85); font-size: 13px;">Bảo mật tài chính thông minh</p>
                        </td>
                    </tr>
                    <!-- Body -->
                    <tr>
                        <td style="padding: 36px 32px;">
                            <h2 style="margin: 0 0 16px; color: #111827; font-size: 20px; font-weight: 700;">Yêu cầu đặt lại mật khẩu</h2>
                            <p style="margin: 0 0 24px; color: #4b5563; font-size: 15px; line-height: 1.6;">
                                Chúng tôi đã nhận được yêu cầu đặt lại mật khẩu cho tài khoản Findora của bạn. Vui lòng sử dụng mã xác thực gồm 6 chữ số dưới đây để hoàn tất:
                            </p>
                            
                            <!-- OTP Box -->
                            <div style="background-color: #f0fdf4; border: 2px dashed #0FA37F; border-radius: 12px; padding: 20px; text-align: center; margin: 24px 0;">
                                <span style="font-family: 'Courier New', Courier, monospace; font-size: 38px; font-weight: 800; letter-spacing: 8px; color: #0FA37F;">{}</span>
                            </div>

                            <p style="margin: 0 0 16px; color: #ef4444; font-size: 14px; font-weight: 600; text-align: center;">
                                ⏱️ Mã xác thực này có hiệu lực trong vòng 120 giây (2 phút).
                            </p>

                            <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 28px 0;">

                            <p style="margin: 0; color: #6b7280; font-size: 13px; line-height: 1.5;">
                                🛡️ <strong>Lưu ý bảo mật:</strong> Nếu bạn không thực hiện yêu cầu này, vui lòng bỏ qua email và kiểm tra lại mật khẩu hiện tại của bạn. Tuyệt đối không chia sẻ mã này cho bất kỳ ai.
                            </p>
                        </td>
                    </tr>
                    <!-- Footer -->
                    <tr>
                        <td align="center" style="background-color: #f9fafb; padding: 20px; border-top: 1px solid #f3f4f6;">
                            <p style="margin: 0; color: #9ca3af; font-size: 12px;">© 2026 Findora Finance. All rights reserved.</p>
                        </td>
                    </tr>
                </table>
            </td>
        </tr>
    </table>
</body>
</html>"#,
            code
        ),
        Language::En => format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Password Reset Code</title>
</head>
<body style="margin: 0; padding: 0; background-color: #f4f5f7; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;">
    <table width="100%" border="0" cellspacing="0" cellpadding="0" style="background-color: #f4f5f7; padding: 30px 15px;">
        <tr>
            <td align="center">
                <table width="100%" border="0" cellspacing="0" cellpadding="0" style="max-width: 520px; background-color: #ffffff; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 12px rgba(0,0,0,0.06);">
                    <tr>
                        <td align="center" style="background: linear-gradient(135deg, #0FA37F 0%, #177264 100%); padding: 32px 20px;">
                            <h1 style="margin: 0; color: #ffffff; font-size: 26px; font-weight: 800; letter-spacing: 2px;">FINDORA</h1>
                            <p style="margin: 6px 0 0; color: rgba(255,255,255,0.85); font-size: 13px;">Smart Financial Security</p>
                        </td>
                    </tr>
                    <tr>
                        <td style="padding: 36px 32px;">
                            <h2 style="margin: 0 0 16px; color: #111827; font-size: 20px; font-weight: 700;">Password Reset Request</h2>
                            <p style="margin: 0 0 24px; color: #4b5563; font-size: 15px; line-height: 1.6;">
                                We received a request to reset the password for your Findora account. Please use the 6-digit verification code below to proceed:
                            </p>
                            
                            <div style="background-color: #f0fdf4; border: 2px dashed #0FA37F; border-radius: 12px; padding: 20px; text-align: center; margin: 24px 0;">
                                <span style="font-family: 'Courier New', Courier, monospace; font-size: 38px; font-weight: 800; letter-spacing: 8px; color: #0FA37F;">{}</span>
                            </div>

                            <p style="margin: 0 0 16px; color: #ef4444; font-size: 14px; font-weight: 600; text-align: center;">
                                ⏱️ This verification code expires in 120 seconds (2 minutes).
                            </p>

                            <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 28px 0;">

                            <p style="margin: 0; color: #6b7280; font-size: 13px; line-height: 1.5;">
                                🛡️ <strong>Security Notice:</strong> If you did not request this code, please safely ignore this email. Never share this code with anyone.
                            </p>
                        </td>
                    </tr>
                    <tr>
                        <td align="center" style="background-color: #f9fafb; padding: 20px; border-top: 1px solid #f3f4f6;">
                            <p style="margin: 0; color: #9ca3af; font-size: 12px;">© 2026 Findora Finance. All rights reserved.</p>
                        </td>
                    </tr>
                </table>
            </td>
        </tr>
    </table>
</body>
</html>"#,
            code
        ),
    }
}
