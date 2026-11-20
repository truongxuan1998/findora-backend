-- ========================================================
-- Findora - Supabase PostgreSQL Schema & Initial Migration
-- Dedicated Isolated Schema: "findora"
-- Hoàn toàn tách biệt với "public", KHÔNG ảnh hưởng dự án khác
-- Copy & Run this script in your Supabase SQL Editor
-- ========================================================

-- 0. Tự động chuyển đổi từ finora sang findora (nếu đã tạo trước đó) hoặc tạo mới
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.schemata WHERE schema_name = 'finora') 
       AND NOT EXISTS (SELECT 1 FROM information_schema.schemata WHERE schema_name = 'findora') THEN
        ALTER SCHEMA finora RENAME TO findora;
    ELSE
        CREATE SCHEMA IF NOT EXISTS findora;
    END IF;
END $$;

-- 1. Bảng Users (Tài khoản người dùng Findora)
CREATE TABLE IF NOT EXISTS findora.users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    full_name TEXT NOT NULL,
    phone_number TEXT DEFAULT '',
    password_hash TEXT NOT NULL,
    avatar_url TEXT DEFAULT '',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_users_email ON findora.users(email);

-- 2. Bảng Categories (Phân loại Chi tiêu & Nguồn thu nhập)
CREATE TABLE IF NOT EXISTS findora.categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    color TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('EXPENSE', 'INCOME')),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- 3. Bảng Wallets (Ví tài chính - Tham chiếu findora.users)
CREATE TABLE IF NOT EXISTS findora.wallets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES findora.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'VND',
    initial_balance BIGINT NOT NULL DEFAULT 0,
    current_balance BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_wallets_user ON findora.wallets(user_id);

-- Self-healing migration cho bảng wallets nếu trước đó lỡ tạo user_id UUID references auth.users
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'findora' AND table_name = 'wallets' AND column_name = 'user_id' AND data_type = 'uuid'
    ) THEN
        ALTER TABLE findora.wallets DROP CONSTRAINT IF EXISTS wallets_user_id_fkey;
        ALTER TABLE findora.wallets ALTER COLUMN user_id TYPE TEXT;
        ALTER TABLE findora.wallets ADD CONSTRAINT wallets_user_id_fkey FOREIGN KEY (user_id) REFERENCES findora.users(id) ON DELETE CASCADE;
    END IF;
END $$;

-- 4. Bảng Transactions (Số nguyên int64 minor units, tuyệt đối không dùng float)
CREATE TABLE IF NOT EXISTS findora.transactions (
    id TEXT PRIMARY KEY,
    wallet_id TEXT NOT NULL REFERENCES findora.wallets(id) ON DELETE CASCADE,
    amount BIGINT NOT NULL CHECK (amount > 0),
    currency VARCHAR(10) NOT NULL DEFAULT 'VND',
    tx_type VARCHAR(20) NOT NULL CHECK (tx_type IN ('EXPENSE', 'INCOME', 'TRANSFER')),
    category_id TEXT NOT NULL REFERENCES findora.categories(id),
    timestamp BIGINT NOT NULL,          -- Unix millis
    note TEXT DEFAULT '',
    created_at BIGINT NOT NULL,         -- Unix millis
    updated_at BIGINT NOT NULL,         -- Unix millis
    deleted_at BIGINT,                  -- Soft delete tombstone
    sync_version BIGINT NOT NULL DEFAULT 1
);

-- 5. High-Performance Covering Indexes trong Schema findora
CREATE INDEX IF NOT EXISTS idx_tx_wallet_timestamp 
    ON findora.transactions(wallet_id, timestamp DESC) 
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tx_category 
    ON findora.transactions(category_id, timestamp DESC) 
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tx_sync_version 
    ON findora.transactions(wallet_id, sync_version);

-- 6. Seed Danh mục mặc định (Chi tiêu & Nguồn thu nhập)
INSERT INTO findora.categories (id, name, icon, color, type) VALUES
    -- Phân loại Chi tiêu
    ('cat_food', 'Ăn uống', 'fork.knife', '#FF5722', 'EXPENSE'),
    ('cat_transport', 'Đi lại', 'car.fill', '#2196F3', 'EXPENSE'),
    ('cat_shopping', 'Mua sắm', 'bag.fill', '#E91E63', 'EXPENSE'),
    ('cat_bills', 'Hóa đơn & Tiện ích', 'doc.text.fill', '#FF9800', 'EXPENSE'),
    ('cat_entertainment', 'Giải trí', 'gamecontroller.fill', '#9C27B0', 'EXPENSE'),
    -- Phân loại Nguồn thu nhập
    ('cat_salary', 'Lương cố định', 'banknote.fill', '#4CAF50', 'INCOME'),
    ('cat_freelance', 'Freelance & Dự án', 'laptopcomputer', '#009688', 'INCOME'),
    ('cat_investment', 'Đầu tư & Cổ tức', 'chart.line.uptrend.xyaxis', '#3F51B5', 'INCOME'),
    ('cat_bonus', 'Thưởng & Quà tặng', 'gift.fill', '#E91E63', 'INCOME')
ON CONFLICT (id) DO NOTHING;

-- 7. Row Level Security (RLS) bảo vệ dữ liệu
ALTER TABLE findora.categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE findora.wallets ENABLE ROW LEVEL SECURITY;
ALTER TABLE findora.transactions ENABLE ROW LEVEL SECURITY;

DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies 
        WHERE schemaname = 'findora' AND tablename = 'categories' AND policyname = 'Allow public read categories'
    ) THEN
        CREATE POLICY "Allow public read categories" ON findora.categories FOR SELECT USING (true);
    END IF;
END $$;
