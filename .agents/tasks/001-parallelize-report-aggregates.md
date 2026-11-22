# [001] Song song hoá các truy vấn trong `fetch_report_aggregates`

- **Người tạo (architect):** Claude
- **Người thực hiện (implementer):** Antigravity
- **Độ ưu tiên:** Trung bình
- **Trạng thái:** TODO

## 1. Mục tiêu / Lý do (Why)
Hàm `fetch_report_aggregates` trong `src/routes/reports.rs` chạy **3 truy vấn độc lập tuần tự** (`await` lần lượt):
1. Totals (SUM theo `tx_type`)
2. Expense breakdown (GROUP BY category, `tx_type = 'EXPENSE'`)
3. Income breakdown (GROUP BY category, `tx_type = 'INCOME'`)

Ba query không phụ thuộc nhau về dữ liệu → đang tốn **3 round-trip DB nối tiếp** mỗi lần cache miss (Supabase pooler ở Singapore). Chạy song song bằng `tokio::try_join!` cắt còn ~1 round-trip, giảm latency các endpoint `/api/reports/monthly`, `/summary`, `/statistics` khi cache miss.

## 2. Phạm vi
**Được đụng vào:**
- `src/routes/reports.rs` — **chỉ** thân hàm `async fn fetch_report_aggregates(...)`

**Ngoài phạm vi (KHÔNG đổi):**
- Chữ ký hàm `fetch_report_aggregates` (tham số, kiểu trả về `(i64, i64, Vec<CategorySummary>, Vec<CategorySummary>, Vec<(i64, String, i64)>)`)
- **Nội dung SQL** của 3 query (giữ nguyên tuyệt đối, kể cả khoảng trắng cuối dòng)
- Logic tính `percentage`, mapping `CategorySummary`, và query thứ 4 `tx_points` (giữ chạy sau, vẫn có điều kiện `if fetch_tx_points`)
- Các handler gọi hàm này và format JSON trả về

## 3. Các bước thực hiện
1. Thay 3 khối `let xxx_rows = sqlx::query(...)...fetch_all(pool).await.map_err(...)?;` thành **3 future** (bỏ `.await.map_err(...)?`, kết thúc ở `.fetch_all(pool)`), đặt tên ví dụ `totals_q`, `expense_q`, `income_q`.
2. Gộp: `let (totals_rows, expense_rows, income_rows) = tokio::try_join!(totals_q, expense_q, income_q).map_err(AppError::Database)?;`
3. Giữ nguyên thứ tự xử lý: **tính `total_income` / `total_expense` từ `totals_rows` TRƯỚC**, vì `expense_breakdown` và `income_breakdown` dùng chúng để tính `percentage`.
4. Sau đó map `expense_breakdown` (từ `expense_rows`) và `income_breakdown` (từ `income_rows`) y như cũ.
5. Query `tx_points` (`if fetch_tx_points { ... }`) giữ nguyên, chạy **sau** try_join.
6. Lưu ý: `pool` là `&sqlx::PgPool`; 3 future dùng chung `&pool`, các tham số bind (`start_millis`, `end_millis`, `user_id`, `wallet_id`) là borrow bất biến → an toàn khi poll đồng thời.

## 4. Tiêu chí nghiệm thu (Acceptance Criteria)
- [ ] 3 query totals/expense/income chạy qua `tokio::try_join!` (không còn 3 `await` tuần tự)
- [ ] Kết quả JSON của `/api/reports/monthly`, `/summary`, `/statistics` **giống hệt** trước khi sửa (same totals, breakdown, time-series)
- [ ] `tests/report_statistics_tests.rs` xanh
- [ ] SQL 3 query không bị thay đổi một ký tự nào
- [ ] Không clippy warning mới

## 5. Cổng kiểm chứng (bắt buộc chạy, phải xanh)
```bash
cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
```
Gợi ý so sánh thủ công: gọi cùng một request report trước/sau khi sửa, `diff` hai JSON trả về phải rỗng.

## 6. Bàn giao
- **Branch:** `perf/parallel-report-aggregates`
- **Commit/PR:** <điền khi xong>
- **Ghi chú của implementer:** <điền>
- **Nhận xét review của Claude:** <để trống>
