# [004] Thu hồi JWT (token_version) + gia hạn phiên + đăng xuất mọi thiết bị

- **Người tạo (architect):** Claude
- **Người thực hiện (implementer):** Claude (theo yêu cầu trực tiếp của người dùng)
- **Độ ưu tiên:** Cao
- **Trạng thái:** REVIEW

## 1. Mục tiêu / Lý do (Why)
JWT không trạng thái, hạn 30 ngày, và backend không có cách thu hồi:
- `logout_handler` chỉ ghi log.
- Đặt lại mật khẩu không vô hiệu token cũ → người dùng nghi lộ tài khoản đổi mật khẩu nhưng token cũ
  (kể cả bản sao Face ID trên máy khác) vẫn dùng được tới 30 ngày.
- App iOS không có cách gia hạn token → cứ 30 ngày Face ID lại bắt nhập mật khẩu.

## 2. Phạm vi
**Được đụng vào:**
- `src/db.rs` — thêm cột `findora.users.token_version INT NOT NULL DEFAULT 0` (self-healing).
- `src/auth.rs` — claim `ver` (`#[serde(default)]`), `create_versioned_jwt`, `check_token_version`,
  `fetch_token_version`; extractor `AuthenticatedUser` so khớp version với DB.
- `src/routes/auth.rs` — login/register/reset cấp token có version; reset tăng version;
  thêm `logout_all_handler`, `refresh_token_handler`.
- `src/routes/mod.rs`, `src/i18n.rs`, `README.md`, `tests/auth_tests.rs`, `tests/token_revocation_tests.rs`.

**Ngoài phạm vi (KHÔNG đổi):**
- Format JSON của các endpoint hiện có; chữ ký `create_jwt` (giữ nguyên, = version 0).
- `POST /api/auth/logout` vẫn KHÔNG thu hồi token (để app mở lại bằng Face ID sau khi đăng xuất).

## 3. Các bước thực hiện
1. Migration cột `token_version`.
2. Claim `ver` mặc định 0 → token cũ vẫn hợp lệ (không đăng xuất hàng loạt khi deploy).
3. Extractor: sau `verify_jwt`, đọc `token_version` theo `id` (PK) và so khớp; user bị xoá → 401.
4. `reset_password_handler`: `token_version = token_version + 1` trong cùng câu UPDATE, cấp token version mới.
5. `POST /api/auth/logout-all`: tăng version. `POST /api/auth/refresh`: cấp token mới 30 ngày cùng version.

## 4. Tiêu chí nghiệm thu (Acceptance Criteria)
- [x] Token cũ không có claim `ver` vẫn verify được và khớp version 0
- [x] Token có version lệch DB hoặc user không tồn tại bị 401
- [x] Đặt lại mật khẩu / logout-all thu hồi mọi token cũ
- [x] `/api/auth/refresh` trả `{ success, message, token }`
- [x] Không phát sinh clippy warning mới
- [x] Không đổi format API hiện có

## 5. Cổng kiểm chứng (bắt buộc chạy, phải xanh)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```

## 6. Bàn giao
- **Branch:** `feat/token-revocation-refresh`
- **Commit/PR:** xem `git log` của branch
- **Ghi chú của implementer:**
  - Extractor thêm 1 truy vấn theo khóa chính mỗi request có xác thực. Nếu cần giảm tải có thể cache
    `token_version` trong Redis TTL ngắn (chưa làm).
  - Cổng kiểm chứng xanh: `cargo build`, `cargo clippy -- -D warnings` (và `--all-targets`),
    `cargo test` 31/31 (người dùng đã đồng ý chạy migration lên Supabase trong `.env` → cột `token_version`
    đã tồn tại trên DB đó), `graphify update .`.
  - `tests/token_revocation_tests.rs` kiểm thử trên DB thật: logout-all thu hồi token cũ, refresh cấp version mới,
    user bị xoá → token bị từ chối.
  - Thứ tự deploy: backend trước, app sau (app mới tự bỏ qua khi backend cũ trả 404 cho `/refresh`).
- **Nhận xét review của Claude:**
