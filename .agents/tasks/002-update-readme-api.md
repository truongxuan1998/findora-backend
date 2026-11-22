# [002] Cập nhật README cho khớp API hiện tại

- **Người tạo (architect):** Claude
- **Người thực hiện (implementer):** Antigravity
- **Độ ưu tiên:** Thấp
- **Trạng thái:** REVIEW

## 1. Mục tiêu / Lý do (Why)
`README.md` hiện liệt kê chỉ ~5 endpoint cũ, thiếu khoảng 15 endpoint, và mô tả tech/deploy lỗi thời. Cần viết lại cho khớp code hiện tại để người dùng/đồng đội tích hợp đúng.

## 2. Phạm vi
**Được đụng vào:**
- `README.md` (viết lại toàn bộ)

**Ngoài phạm vi (KHÔNG đổi):**
- Bất kỳ file code nào (`src/**`), `Cargo.toml`, `render.yaml`, `.env*`

## 3. Nguồn sự thật (đọc trước khi viết)
- `src/routes/mod.rs` — danh sách route chính xác
- `src/config.rs` — biến môi trường
- Các handler trong `src/routes/*` — endpoint nào yêu cầu auth (extractor `AuthenticatedUser`)
- `render.yaml`, `Dockerfile` — cấu hình deploy

## 4. Nội dung README mới (bắt buộc có)

### 4.1 Bảng endpoint đầy đủ (kèm cột Auth)
| Method | Path | Auth | Mô tả |
|---|---|---|---|
| GET | `/` | ❌ | Thông tin service |
| GET | `/health` | ❌ | Health check DB + Redis |
| POST | `/api/auth/register` | ❌ | Đăng ký + tạo ví mặc định |
| POST | `/api/auth/login` | ❌ | Đăng nhập (lockout sau 8 lần sai) |
| POST | `/api/auth/logout` | ✅ | Đăng xuất |
| POST | `/api/auth/forgot-password` | ❌ | Gửi OTP 6 số (hết hạn 120s) |
| POST | `/api/auth/reset-password` | ❌ | Đặt lại mật khẩu bằng OTP |
| GET | `/api/auth/me` | ✅ | Thông tin user hiện tại |
| GET/PUT | `/api/users/profile` | ✅ | Xem / cập nhật hồ sơ |
| POST | `/api/users/avatar` | ✅ | Upload avatar (multipart) |
| GET | `/api/categories` | ❌ | Danh mục (song ngữ) |
| GET/POST | `/api/wallets` | ✅ | Danh sách / tạo ví |
| GET/POST/DELETE | `/api/transactions` | ✅ | Liệt kê / tạo / xoá sạch giao dịch |
| DELETE/PATCH | `/api/transactions/:id` | ✅ | Xoá mềm / sửa ghi chú |
| GET | `/api/reports/monthly` | ✅ | Báo cáo tháng |
| GET | `/api/reports/summary` | ✅ | Báo cáo theo week/month/year |
| GET | `/api/reports/statistics` | ✅ | (alias của summary) |
| GET | `/api/reports/category/:id` | ✅ | Lịch sử danh mục theo tháng |
| GET/POST | `/api/translate` | ❌ | Dịch văn bản (vi↔en) |
| POST | `/api/transactions/translate` | ✅ | Dịch ghi chú giao dịch |

### 4.2 Tech stack thực tế
Rust · Axum · Tokio · SQLx · Supabase PostgreSQL (schema `findora`) · Redis dual-engine (Upstash TCP + REST fallback) · Email Resend → Brevo fallback · bcrypt (chạy qua `spawn_blocking`) · JWT 30 ngày · i18n qua header `Accept-Language` (vi/en) · rate-limit phân tán qua Redis · validate ảnh bằng magic-bytes · giao dịch TRANSFER 2 chiều (ví nguồn → ví đích).

### 4.3 Biến môi trường
`DATABASE_URL` (bắt buộc) · `APP_ENV` (development/production) · `JWT_SECRET` (**bắt buộc ở production**, không có default) · `PORT` · `SUPABASE_URL` · `SUPABASE_SERVICE_KEY`/`SUPABASE_KEY` · `SUPABASE_STORAGE_BUCKET` · `REDIS_URL` · `UPSTASH_REDIS_REST_URL` · `UPSTASH_REDIS_REST_TOKEN` · `RESEND_API_KEY` · `RESEND_FROM` · `BREVO_API_KEY` · `BREVO_SENDER_EMAIL` · `BREVO_SENDER_NAME`. (Tham chiếu `.env.example`.)

### 4.4 Header hỗ trợ
- `Authorization: Bearer <jwt>` cho endpoint cần auth
- `Accept-Language: vi | en` chọn ngôn ngữ phản hồi
- Audit (tuỳ chọn): `X-Device-Model`, `X-Device-Name`, `X-Device-OS`, `X-Device-Location`; tự nhận `cf-connecting-ip`, `cf-ipcity`, `cf-ipcountry`

### 4.5 Rate limit
- Nhóm nhạy cảm (`/api/auth/*`, `/api/users/avatar`): **20 req/phút/IP**
- API chung: **120 req/phút/IP**
- Trả header `X-RateLimit-*` và `Retry-After`

### 4.6 Chạy local & Deploy
- Local: `cp .env.example .env` → điền `DATABASE_URL` + `JWT_SECRET` (hoặc `APP_ENV=development`) → `cargo run`
- Deploy: Docker trên Render (xem `render.yaml`); `JWT_SECRET` dùng `generateValue`

## 5. Tiêu chí nghiệm thu (Acceptance Criteria)
- [x] Mọi endpoint trong README khớp 1-1 với `src/routes/mod.rs` (không thừa, không thiếu)
- [x] Cột Auth đúng với extractor trong handler
- [x] Danh sách env khớp `src/config.rs` / `.env.example`
- [x] Không còn thông tin sai (số endpoint, tech, deploy)
- [x] Chỉ `README.md` thay đổi

## 6. Cổng kiểm chứng (bắt buộc chạy, phải xanh)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```
(README không ảnh hưởng build; cổng này để đảm bảo không vô tình đụng file khác.)

## 7. Bàn giao
- **Branch:** `docs/update-readme-api`
- **Commit/PR:** 137bc11
- **Ghi chú của implementer:** Đã viết lại README.md hoàn chỉnh: bảng 20 endpoints khớp 1-1 với `src/routes/mod.rs` kèm trạng thái Auth, tech stack thực tế, biến môi trường, headers hỗ trợ (audit, i18n, auth), chính sách rate-limiting và hướng dẫn run local / deploy Render. Cổng kiểm chứng đã chạy và pass toàn bộ.
- **Nhận xét review của Claude:** <để trống>
