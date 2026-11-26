# Findora Backend (Rust + Axum + Supabase)

Dịch vụ backend hiệu năng cao, tối ưu bộ nhớ cho ứng dụng quản lý tài chính cá nhân **Findora**.

## 🚀 Tech Stack Thực Tế

- **Ngôn ngữ & Runtime:** Rust · Axum · Tokio async runtime.
- **Cơ sở dữ liệu:** Supabase PostgreSQL với schema riêng biệt `findora` (cách ly hoàn toàn khỏi `public`).
- **ORM / Query:** SQLx (chạy migration tự động, compile-time/runtime query verification).
- **Caching & Rate Limit:** Redis dual-engine (Upstash TCP connection pool + fallback REST API) hỗ trợ rate limit phân tán khi scale nhiều instance.
- **Giao dịch tài chính:** Hỗ trợ giao dịch `TRANSFER` 2 chiều (ví nguồn ➜ ví đích đồng bộ).
- **Email:** Transactional email với cơ chế fallback tự động Resend ➜ Brevo (Sendinblue).
- **Bảo mật & Auth:**
  - Password hashing bằng `bcrypt` (chạy trên threadpool tách biệt qua `tokio::task::spawn_blocking`).
  - JWT token thời hạn 30 ngày.
  - Rate limiting phân tán qua Redis: lockout sau 8 lần đăng nhập sai.
  - Validate file upload (avatar) bằng magic-bytes (`image/jpeg`, `image/png`, `image/webp`), từ chối file giả mạo hoặc vượt giới hạn dung lượng.
- **Quốc tế hoá (i18n):** Song ngữ qua header `Accept-Language` (`vi` hoặc `en`), trả về thông điệp và danh mục song ngữ.
- **Dịch thuật (Translation):** Hỗ trợ dịch tự động ghi chú và nội dung tài chính hai chiều vi ↔ en kết hợp cache Redis.
- **Audit Logging:** Tự động ghi nhận thông tin thiết bị, IP và vị trí địa lý cho mọi request.

---

## 🌐 Live Production URL

- **Production API:** `https://findora-backend-zpr5.onrender.com`
- **Health Check:** `https://findora-backend-zpr5.onrender.com/health`
- **Region:** Singapore (Cloudflare CDN + Render Docker)

---

## 📡 Danh Sách API Endpoints

| Method | Path | Auth | Mô tả |
|---|---|---|---|
| GET | `/` | ❌ | Thông tin service Findora Backend |
| GET | `/health` | ❌ | Health check kết nối DB Supabase & Redis |
| POST | `/api/auth/register` | ❌ | Đăng ký tài khoản mới + tạo ví mặc định |
| POST | `/api/auth/login` | ❌ | Đăng nhập lấy JWT (lockout sau 8 lần sai) |
| POST | `/api/auth/logout` | ✅ | Đăng xuất trên thiết bị hiện tại (token vẫn còn hạn để mở lại bằng Face ID) |
| POST | `/api/auth/logout-all` | ✅ | Đăng xuất mọi thiết bị: thu hồi toàn bộ JWT đã cấp (tăng `token_version`) |
| POST | `/api/auth/refresh` | ✅ | Đổi token còn hiệu lực lấy token mới hạn 30 ngày (`{ success, message, token }`) |
| POST | `/api/auth/forgot-password` | ❌ | Gửi OTP 6 số qua email (hết hạn trong 120s) |
| POST | `/api/auth/reset-password` | ❌ | Đặt lại mật khẩu bằng mã OTP — thu hồi mọi token cũ, trả về token mới |
| GET | `/api/auth/me` | ✅ | Lấy thông tin user hiện tại |
| GET | `/api/users/profile` | ✅ | Xem thông tin hồ sơ người dùng |
| PUT | `/api/users/profile` | ✅ | Cập nhật hồ sơ người dùng |
| POST | `/api/users/avatar` | ✅ | Upload avatar (multipart form-data, kiểm tra magic-bytes) |
| GET | `/api/categories` | ❌ | Danh sách danh mục thu/chi (song ngữ vi/en) |
| GET | `/api/wallets` | ✅ | Danh sách ví của người dùng |
| POST | `/api/wallets` | ✅ | Tạo ví mới |
| GET | `/api/transactions` | ✅ | Liệt kê danh sách giao dịch (lọc theo ví, ngày, loại) |
| POST | `/api/transactions` | ✅ | Tạo giao dịch thu/chi hoặc chuyển khoản 2 chiều (`TRANSFER`) |
| DELETE | `/api/transactions` | ✅ | Xoá toàn bộ giao dịch của ví / tài khoản |
| DELETE | `/api/transactions/:id` | ✅ | Xoá mềm giao dịch (soft delete) |
| PATCH | `/api/transactions/:id` | ✅ | Cập nhật ghi chú giao dịch |
| GET | `/api/reports/monthly` | ✅ | Báo cáo tài chính theo tháng (thu, chi, phân loại danh mục) |
| GET | `/api/reports/summary` | ✅ | Báo cáo tài chính theo chu kỳ (`week`, `month`, `year`) |
| GET | `/api/reports/statistics` | ✅ | Alias của `/api/reports/summary` |
| GET | `/api/reports/category/:id` | ✅ | Báo cáo lịch sử biến động theo từng danh mục qua các tháng |
| GET | `/api/translate` | ❌ | Dịch văn bản song ngữ vi ↔ en qua query param |
| POST | `/api/translate` | ❌ | Dịch văn bản song ngữ vi ↔ en qua request body |
| POST | `/api/transactions/translate` | ✅ | Dịch hàng loạt ghi chú giao dịch của người dùng |

---

## 🛡️ Header Hỗ Trợ

- **Xác thực:**
  - `Authorization: Bearer <jwt_token>`: Bắt buộc đối với các endpoint có cờ Auth ✅.
- **Ngôn ngữ:**
  - `Accept-Language: vi` (mặc định) hoặc `Accept-Language: en`: Định danh ngôn ngữ phản hồi thông điệp lỗi và danh mục.
- **Audit Telemetry (Tuỳ chọn cho client):**
  - `X-Device-Model`: Model thiết bị (ví dụ `iPhone15,2`, `SM-S918B`).
  - `X-Device-Name`: Tên thiết bị do người dùng đặt.
  - `X-Device-OS`: Hệ điều hành (ví dụ `iOS 17.4`, `Android 14`).
  - `X-Device-Location`: Toạ độ hoặc mô tả vị trí thủ công.
  - *Cloudflare Headers tự động nhận diện:* `cf-connecting-ip`, `cf-ipcity`, `cf-ipcountry`.

---

## ⏱️ Rate Limiting

Hệ thống áp dụng giới hạn tần suất request theo IP (lưu trữ phân tán qua Redis):
- **Nhóm nhạy cảm (`/api/auth/*`, `/api/users/avatar`):** Tối đa **20 req/phút/IP**.
- **API chung:** Tối đa **120 req/phút/IP**.
- **Phản hồi:** Trả kèm các header chuẩn `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`, và `Retry-After` (khi bị HTTP 429 Too Many Requests).

---

## ⚙️ Biến Môi Trường (Environment Variables)

Tham chiếu chi tiết tại [.env.example](file://.env.example):

| Biến | Bắt buộc | Mô tả |
|---|---|---|
| `DATABASE_URL` | Có | Chuỗi kết nối Supabase PostgreSQL (Postgres pooler, SSL require). |
| `APP_ENV` | Không | Môi trường chạy: `development` (mặc định) hoặc `production`. |
| `JWT_SECRET` | **Có (Prod)** | Khóa bí mật ký JWT (tối thiểu 32 ký tự). Bắt buộc ở production, không có mặc định. |
| `PORT` | Không | Cổng HTTP server lắng nghe (mặc định: `10000`). |
| `SUPABASE_URL` | Không | URL project Supabase cho Storage bucket. |
| `SUPABASE_SERVICE_KEY` / `SUPABASE_KEY` | Không | Service role key hoặc anon key để upload Storage. |
| `SUPABASE_STORAGE_BUCKET` | Không | Tên bucket lưu avatar (mặc định: `findora_avatar_user`). |
| `REDIS_URL` | Không | Redis TCP connection URL (Upstash Redis protocol `rediss://`). |
| `UPSTASH_REDIS_REST_URL` | Không | Upstash Redis REST URL (fallback engine). |
| `UPSTASH_REDIS_REST_TOKEN` | Không | Upstash Redis REST Token (fallback engine). |
| `RESEND_API_KEY` | Không | API Key của dịch vụ gửi email Resend. |
| `RESEND_FROM` | Không | Địa chỉ gửi mail Resend (mặc định: `Findora Security <onboarding@resend.dev>`). |
| `BREVO_API_KEY` | Không | API Key của dịch vụ gửi email Brevo (fallback). |
| `BREVO_SENDER_EMAIL` | Không | Email người gửi xác thực trên Brevo. |
| `BREVO_SENDER_NAME` | Không | Tên hiển thị người gửi Brevo (mặc định: `Findora Security`). |

---

## 🛠 Local Development

```bash
# 1. Sao chép file cấu hình môi trường mẫu
cp .env.example .env

# 2. Cấu hình DATABASE_URL và JWT_SECRET (hoặc để APP_ENV=development)
# Chỉnh sửa file .env với thông tin kết nối DB của bạn

# 3. Khởi chạy server backend
cargo run
```

---

## ☁️ Deploy lên Render.com

Hệ thống được đóng gói Docker tối ưu (multi-stage build, runtime Debian bookworm slim ~35MB, chạy user non-root `findora`):

1. Khởi tạo Web Service trên [Render Dashboard](https://dashboard.render.com) kết nối với repository.
2. Chọn môi trường **Docker** (Render tự động đọc [Dockerfile](file://Dockerfile) và [render.yaml](file://render.yaml)).
3. Cấu hình biến môi trường:
   - `DATABASE_URL`: Connection string Supabase PostgreSQL.
   - `JWT_SECRET`: Render tự động sinh giá trị an toàn (`generateValue: true`) hoặc thiết lập thủ công.
   - Điền các biến Redis / Email tương ứng nếu có.
4. Deploy và kiểm tra trạng thái tại endpoint `/health`.
