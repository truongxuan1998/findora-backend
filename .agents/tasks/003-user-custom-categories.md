# [003] Danh mục chi tiêu tuỳ chỉnh theo người dùng (tạo & lưu được giao dịch)

- **Người tạo (architect):** Claude
- **Người thực hiện (implementer):** Antigravity
- **Độ ưu tiên:** Cao
- **Trạng thái:** TODO

## 1. Mục tiêu / Lý do (Why)
Hiện người dùng **không lưu được giao dịch cho danh mục mới tự tạo**. Nguyên nhân:
- Backend **không có API tạo danh mục** — chỉ `GET /api/categories`. Danh mục là 9 mục global cố định seed trong `db.rs`, bảng `findora.categories` **không có `user_id`**.
- `transactions.category_id` có khóa ngoại cứng `REFERENCES findora.categories(id)` (`db.rs:152`).
- Danh mục mới chỉ tồn tại phía app → khi lưu giao dịch với `category_id` đó, Postgres báo **vi phạm FK (mã 23503)**. `errors.rs` chỉ xử lý 23505 → trả **HTTP 500** mơ hồ, giao dịch không được lưu.

**Kết quả mong muốn:** user tạo được danh mục riêng qua API, danh mục đó lưu vào DB, và giao dịch tham chiếu nó lưu thành công; nếu `category_id` không hợp lệ thì trả lỗi 400 rõ ràng thay vì 500.

## 2. Phạm vi
**Được đụng vào:**
- `src/db.rs` — thêm cột `user_id` + index cho `findora.categories` (self-healing)
- `src/models/category.rs` — thêm `CreateCategoryRequest`
- `src/routes/categories.rs` — sửa `GET` (global + của user) và thêm handler `POST`
- `src/routes/mod.rs` — thêm route `POST /api/categories`
- `src/errors.rs` — map mã 23503 (FK violation) → `BadRequest`
- `src/routes/transactions.rs` — `create_transaction_handler`: xác thực `category_id` là global hoặc thuộc user
- `tests/` — thêm test cho luồng mới

**Ngoài phạm vi (KHÔNG đổi):**
- Format JSON của `CategoryItem` hiện có (không bỏ/đổi field đang có; chỉ được thêm field optional nếu cần)
- 9 danh mục mặc định (giữ nguyên, `user_id = NULL`)
- Các route/handler khác không liên quan

## 3. Các bước thực hiện

### 3.1 Schema (`db.rs`, trong `run_schema_and_migrations`)
- Thêm vào phần tạo bảng categories (self-healing, chạy mỗi boot):
  ```sql
  ALTER TABLE findora.categories ADD COLUMN IF NOT EXISTS user_id TEXT;
  CREATE INDEX IF NOT EXISTS idx_categories_user ON findora.categories(user_id);
  ```
  - `user_id = NULL` → danh mục mặc định global; khác NULL → danh mục riêng của user.
  - (Tuỳ chọn) thêm FK `user_id REFERENCES findora.users(id) ON DELETE CASCADE` qua khối `DO $$ ... IF NOT EXISTS (pg_constraint) ... $$` để xoá user thì xoá danh mục riêng. Nếu làm, phải idempotent.

### 3.2 Model (`models/category.rs`)
```rust
#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    #[serde(default, alias = "nameEn")]
    pub name_en: Option<String>,
    pub icon: String,
    pub color: String,
    pub r#type: String, // "EXPENSE" | "INCOME"
}
```

### 3.3 `GET /api/categories` (yêu cầu auth) — `categories.rs`
- Đổi handler nhận `auth: AuthenticatedUser` (hiện đang public). **Lưu ý: đây là đổi contract — client phải gửi Bearer token.** Ghi rõ trong mục Bàn giao.
- Query: `WHERE user_id IS NULL OR user_id = $1` → trả **mặc định + của chính user**, `ORDER BY type, id`.
- **Cache key phải kèm user_id**: `findora:cache:categories:{user_id}:{lang}` (hiện chỉ theo lang → sẽ rò danh mục giữa các user nếu không sửa).

### 3.4 `POST /api/categories` (auth) — `categories.rs` + route trong `mod.rs`
- Validate: `name`, `icon`, `color` không rỗng; `type.to_uppercase()` ∈ {`EXPENSE`,`INCOME`} (nếu không → `BadRequest`).
- `id = format!("cat_{}", Uuid::new_v4().simple())`, `user_id = auth.user_id`.
- `INSERT ... RETURNING ...`, trả `(StatusCode::CREATED, Json(CategoryItem))`.
- Invalidate cache: `state.redis.del_pattern(&format!("findora:cache:categories:{}*", auth.user_id)).await;`
- Route: `.route("/api/categories", get(...).post(categories::create_category_handler))`.

### 3.5 Lỗi FK rõ ràng (`errors.rs`)
Trong `From<sqlx::Error>`, thêm nhánh cho FK:
```rust
if db_err.code().as_deref() == Some("23503") {
    return AppError::BadRequest("Danh mục không tồn tại hoặc không hợp lệ".into());
}
```
(đặt cạnh nhánh 23505 hiện có).

### 3.6 Xác thực quyền sở hữu danh mục khi tạo giao dịch (`transactions.rs`)
Trong `create_transaction_handler`, trước khi insert, kiểm tra `category_id` **là global (`user_id IS NULL`) hoặc thuộc user**:
```sql
SELECT 1 FROM findora.categories WHERE id = $1 AND (user_id IS NULL OR user_id = $2)
```
Nếu không có → `AppError::BadRequest("Danh mục không hợp lệ")`. (Tránh dùng danh mục riêng của user khác dù FK vẫn pass.)

## 4. Tiêu chí nghiệm thu (Acceptance Criteria)
- [ ] `POST /api/categories` (có token) tạo được danh mục, trả 201 + `CategoryItem` có `id`
- [ ] `GET /api/categories` trả **9 mặc định + danh mục riêng của user**, không thấy danh mục riêng của user khác
- [ ] Tạo giao dịch với `category_id` vừa tạo → **lưu thành công**, số dư ví cập nhật đúng
- [ ] Tạo giao dịch với `category_id` không tồn tại → **HTTP 400** "Danh mục không tồn tại…" (không phải 500)
- [ ] 9 danh mục mặc định vẫn hiển thị cho mọi user
- [ ] Cache danh mục tách theo user (không rò giữa các user); POST invalidate đúng
- [ ] Có test bao phủ: tạo category → tạo transaction với category đó → liệt kê thấy giao dịch
- [ ] Không clippy warning mới; không đổi format field `CategoryItem` đang có

## 5. Cổng kiểm chứng (bắt buộc chạy, phải xanh)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```

## 6. Bàn giao
- **Branch:** `feat/user-categories`
- **Commit/PR:** <điền khi xong>
- **Ghi chú của implementer:** <điền — nhớ nêu rõ nếu đã đổi GET /api/categories sang yêu cầu auth (ảnh hưởng client)>
- **Nhận xét review của Claude:** <để trống>
