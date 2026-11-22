# Task Cards — Quy trình "Claude nghĩ · Antigravity code"

Thư mục này chứa các **task card** (đặc tả công việc) để bàn giao giữa các agent.

## Phân vai
- **Claude** = kiến trúc / review / tối ưu. Chạy ở *plan mode* (chỉ đọc), viết task card vào đây, và review diff. **Không** tự implement.
- **Antigravity (Gemini)** = người implement. Đọc card → code trên một branch → tự chạy cổng kiểm chứng → cập nhật mục "Bàn giao" của card.

## Luồng làm việc
1. Claude tạo card từ `TEMPLATE.md`, đặt tên `NNN-ten-ngan.md` (số tăng dần).
2. Antigravity nhận card có trạng thái `TODO`, đổi sang `IN-PROGRESS`, code đúng **Phạm vi**, không đụng **Ngoài phạm vi**.
3. Chạy **Cổng kiểm chứng** — phải xanh toàn bộ mới được chuyển `REVIEW`.
4. Claude review diff (`/code-review`), ghi nhận xét vào mục "Bàn giao"; nếu đạt → `DONE`.

## Cổng kiểm chứng (bắt buộc, mọi card)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```

## Nguyên tắc
- Chỉ **một agent** sửa code tại một thời điểm.
- Mỗi card nhỏ, atomic, có tiêu chí nghiệm thu đo được.
- Không đổi format JSON API / chữ ký public nếu card không yêu cầu.
- Tôn trọng quyền trong `GEMINI.md` (được commit/push `findora-backend`; `findora-app` cần duyệt tay).
