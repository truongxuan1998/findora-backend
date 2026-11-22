# AGENTS.md — findora-backend

Hướng dẫn chung cho mọi AI agent làm việc trên repo này (Claude, Antigravity/Gemini, …).
Phân vai: **Claude** = kiến trúc / review / tối ưu (plan mode, không code). **Antigravity** = người implement.

## Dự án
Backend Rust cho app tài chính cá nhân Findora: Axum · Tokio · SQLx · Supabase PostgreSQL (schema `findora`) · Redis dual-engine (Upstash TCP + REST) · Email Resend→Brevo · JWT · i18n (vi/en).

## Quyền & phạm vi repo
- **findora-backend**: được phép commit/push lên `origin/main` khi người dùng yêu cầu.
- **findora-app**: BẮT BUỘC có phê duyệt rõ ràng của người dùng trước khi sửa/commit/push.

## Task Cards (cách nhận và làm việc)
Công việc được giao qua **task card** trong `.agents/tasks/`.

- "làm task NNN" → mở `.agents/tasks/NNN-*.md` và làm đúng theo card.
- Tuân thủ nghiêm **"Phạm vi"** và **"Ngoài phạm vi"** của card.
- Làm trên **branch** ghi trong card; không đổi format JSON API / chữ ký public nếu card không yêu cầu.
- Chạy **Cổng kiểm chứng** (phải xanh toàn bộ) trước khi báo xong:
  ```bash
  cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
  ```
- Cập nhật mục **"Bàn giao"**, đổi **Trạng thái** sang `REVIEW`, rồi commit. **KHÔNG** tự đánh `DONE` (Claude review mới đóng).
- Card mới: copy `.agents/tasks/TEMPLATE.md`. Quy trình đầy đủ: `.agents/tasks/README.md`.

## Quy ước code
- Tất cả truy vấn SQL dùng tham số hoá (không nối chuỗi).
- Thao tác CPU-bound nặng (bcrypt) chạy qua `tokio::task::spawn_blocking`.
- Nguồn sự thật của schema DB là `src/db.rs::run_schema_and_migrations` (self-healing lúc boot), KHÔNG dùng thư mục migrations rời.
- Giữ clippy sạch (`cargo clippy -- -D warnings`).

## graphify (điều hướng codebase)
- Khi `graphify-out/graph.json` tồn tại, chạy `graphify query "<câu hỏi>"` TRƯỚC khi grep/đọc file cho câu hỏi về codebase; chỉ đọc file thô ở những dòng đã định vị.
- `graphify path "<A>" "<B>"` cho quan hệ; `graphify explain "<concept>"` cho khái niệm; `graphify section "<heading>"` để in 1 mục markdown.
- Sau khi sửa code: `graphify update .` để cập nhật graph.
- `graphify-out/graph.json` bị ghi lại toàn bộ mỗi lần build → diff lớn là bình thường, đừng coi là churn.
