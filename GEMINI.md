## Permissions & Repository Scope

- **findora-backend**: Authorized to commit and push changes directly to `origin/main` when requested.
- **findora-app**: STRICTLY REQUIRES EXPLICIT USER APPROVAL. Do NOT modify, commit, push, or distribute `findora-app` without explicit user confirmation first.

## Task Cards (quy trình bàn giao Claude ↔ Antigravity)

Công việc được giao qua **task card** trong `.agents/tasks/`. Claude (architect) viết card; bạn (Antigravity) là người implement.

- Khi người dùng nói "làm task NNN" (hoặc "thực hiện task NNN"), hãy mở `.agents/tasks/NNN-*.md` và làm **đúng theo card**.
- Tuân thủ nghiêm mục **"Phạm vi"** (chỉ đụng file/hàm được liệt kê) và **"Ngoài phạm vi"** (tuyệt đối không đổi).
- Làm trên **branch** ghi trong card (mục "Bàn giao").
- Trước khi báo xong, **chạy Cổng kiểm chứng và phải xanh toàn bộ**:
  ```bash
  cargo build && cargo clippy -- -D warnings && cargo test && graphify update .
  ```
- Cập nhật mục **"Bàn giao"** của card (branch, commit, ghi chú đã làm gì / lệch gì), đổi **Trạng thái** sang `REVIEW`, rồi commit.
- KHÔNG tự đánh dấu `DONE` — mục đó dành cho Claude sau khi review diff.
- Tạo card mới: copy `.agents/tasks/TEMPLATE.md` → `NNN-ten-ngan.md`. Quy trình đầy đủ ở `.agents/tasks/README.md`.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- When graphify-out/graph.json exists, run `graphify query "<question>"` before your first grep, search or source-file read for a codebase question, and put this rule in every subagent prompt that explores code. Read raw files only for lines the graph or an earlier read already located. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. When the answer is under a Markdown heading, `graphify section "<heading>"` prints that section's text, so the .md need not be opened. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- A committed graphify-out/graph.json is rewritten whole on every build, so a large diff to it in a pull request is expected. Do not flag it as churn or suggest skipping the graph rebuild or its hooks.
