# [NNN] <Tiêu đề ngắn gọn>

> Sao chép file này thành `NNN-ten-ngan.md` khi tạo card mới.

- **Người tạo (architect):** Claude
- **Người thực hiện (implementer):** Antigravity
- **Độ ưu tiên:** Cao / Trung bình / Thấp
- **Trạng thái:** TODO · IN-PROGRESS · REVIEW · DONE

## 1. Mục tiêu / Lý do (Why)
<Vấn đề hiện tại là gì, vì sao cần sửa, kết quả mong muốn sau khi xong.>

## 2. Phạm vi
**Được đụng vào:**
- `path/to/file.rs` — <hàm / vùng cụ thể>

**Ngoài phạm vi (KHÔNG đổi):**
- <Những thứ không được thay đổi: chữ ký public, format JSON, schema DB, file khác...>

## 3. Các bước thực hiện
1. <Bước 1 — tham chiếu hàm/pattern có sẵn nếu tái dùng>
2. <Bước 2>
3. <...>

## 4. Tiêu chí nghiệm thu (Acceptance Criteria)
- [ ] <Tiêu chí đo được 1>
- [ ] <Tiêu chí đo được 2>
- [ ] Không phát sinh clippy warning mới
- [ ] Không đổi hành vi/format API ngoài yêu cầu của card

## 5. Cổng kiểm chứng (bắt buộc chạy, phải xanh)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```

## 6. Bàn giao
- **Branch:** <ten-branch>
- **Commit/PR:** <link hoặc hash>
- **Ghi chú của implementer:** <tóm tắt đã làm gì, lệch gì so với card>
- **Nhận xét review của Claude:** <để trống — Claude điền khi review>
