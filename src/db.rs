use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

pub async fn create_pool(db_url: &str) -> Result<PgPool, sqlx::Error> {
    let connect_opts = PgConnectOptions::from_str(db_url)?
        .options([("timezone", "Asia/Ho_Chi_Minh")])
        .statement_cache_capacity(0); // Bắt buộc vô hiệu hóa statement cache cho Supabase PgBouncer/Transaction Pooler

    let pool = PgPoolOptions::new()
        .max_connections(5) // Tối ưu cho Render Free-Tier (512MB RAM) và Supabase Pooler
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(15))
        .idle_timeout(Duration::from_secs(300))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET TIME ZONE 'Asia/Ho_Chi_Minh'").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with(connect_opts)
        .await?;

    info!("✅ Kết nối Supabase PostgreSQL thành công! (Pool size: max 5, timezone: Asia/Ho_Chi_Minh)");
    Ok(pool)
}

pub async fn run_schema_and_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    info!("🔄 Đang tự động kiểm tra và chuẩn hóa schema cơ sở dữ liệu 'findora'...");

    // 0. Cấu hình Múi giờ Việt Nam (Asia/Ho_Chi_Minh, UTC+7) toàn diện cho DB và Session
    let _ = sqlx::raw_sql(
        "DO $$
        BEGIN
            EXECUTE 'ALTER DATABASE postgres SET timezone TO ''Asia/Ho_Chi_Minh''';
            EXECUTE 'ALTER ROLE postgres SET timezone TO ''Asia/Ho_Chi_Minh''';
        EXCEPTION WHEN OTHERS THEN
            NULL;
        END $$;
        SET TIME ZONE 'Asia/Ho_Chi_Minh';"
    )
    .execute(pool)
    .await;

    // 1. Tạo schema findora
    sqlx::raw_sql("CREATE SCHEMA IF NOT EXISTS findora;")
        .execute(pool)
        .await?;

    // 2. Bảng Users (hỗ trợ Rate Limit sai mật khẩu & Lockout qua ngày)
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS findora.users (
            id TEXT PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            full_name TEXT NOT NULL,
            phone_number TEXT DEFAULT '',
            password_hash TEXT NOT NULL,
            avatar_url TEXT DEFAULT '',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            updated_at TIMESTAMPTZ DEFAULT NOW(),
            failed_login_attempts INT NOT NULL DEFAULT 0,
            locked_until TIMESTAMPTZ,
            last_failed_at TIMESTAMPTZ
        );
        ALTER TABLE findora.users ADD COLUMN IF NOT EXISTS failed_login_attempts INT NOT NULL DEFAULT 0;
        ALTER TABLE findora.users ADD COLUMN IF NOT EXISTS locked_until TIMESTAMPTZ;
        ALTER TABLE findora.users ADD COLUMN IF NOT EXISTS last_failed_at TIMESTAMPTZ;
        CREATE INDEX IF NOT EXISTS idx_users_email ON findora.users(email);",
    )
    .execute(pool)
    .await?;

    // 3. Bảng Wallets
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS findora.wallets (
            id TEXT PRIMARY KEY,
            user_id TEXT REFERENCES findora.users(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            currency VARCHAR(10) NOT NULL DEFAULT 'VND',
            initial_balance BIGINT NOT NULL DEFAULT 0,
            current_balance BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ DEFAULT NOW(),
            updated_at TIMESTAMPTZ DEFAULT NOW(),
            deleted_at TIMESTAMPTZ
        );
        CREATE INDEX IF NOT EXISTS idx_wallets_user ON findora.wallets(user_id);",
    )
    .execute(pool)
    .await?;

    // Self-healing migration cho bảng wallets nếu trước đó lỡ tạo user_id UUID references auth.users
    sqlx::raw_sql(
        "DO $$
        BEGIN
            IF EXISTS (
                SELECT 1 FROM information_schema.columns 
                WHERE table_schema = 'findora' AND table_name = 'wallets' AND column_name = 'user_id' AND data_type = 'uuid'
            ) THEN
                ALTER TABLE findora.wallets DROP CONSTRAINT IF EXISTS wallets_user_id_fkey;
                ALTER TABLE findora.wallets ALTER COLUMN user_id TYPE TEXT;
                ALTER TABLE findora.wallets ADD CONSTRAINT wallets_user_id_fkey FOREIGN KEY (user_id) REFERENCES findora.users(id) ON DELETE CASCADE;
            END IF;
        END $$;",
    )
    .execute(pool)
    .await?;

    // 4. Bảng Categories (Hỗ trợ Song ngữ: name - Tiếng Việt, name_en - Tiếng Anh)
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS findora.categories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            name_en TEXT,
            icon TEXT NOT NULL,
            color TEXT NOT NULL,
            type TEXT NOT NULL CHECK (type IN ('EXPENSE', 'INCOME')),
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
        ALTER TABLE findora.categories ADD COLUMN IF NOT EXISTS name_en TEXT;",
    )
    .execute(pool)
    .await?;

    // Seed và cập nhật tên tiếng Anh cho danh mục mặc định
    sqlx::raw_sql(
        "INSERT INTO findora.categories (id, name, name_en, icon, color, type) VALUES
            ('cat_food', 'Ăn uống', 'Food & Dining', 'fork.knife', '#FF5722', 'EXPENSE'),
            ('cat_transport', 'Đi lại', 'Transportation', 'car.fill', '#2196F3', 'EXPENSE'),
            ('cat_shopping', 'Mua sắm', 'Shopping', 'bag.fill', '#E91E63', 'EXPENSE'),
            ('cat_bills', 'Hóa đơn & Tiện ích', 'Bills & Utilities', 'doc.text.fill', '#FF9800', 'EXPENSE'),
            ('cat_entertainment', 'Giải trí', 'Entertainment', 'gamecontroller.fill', '#9C27B0', 'EXPENSE'),
            ('cat_salary', 'Lương cố định', 'Fixed Salary', 'banknote.fill', '#4CAF50', 'INCOME'),
            ('cat_freelance', 'Freelance & Dự án', 'Freelance & Projects', 'laptopcomputer', '#009688', 'INCOME'),
            ('cat_investment', 'Đầu tư & Cổ tức', 'Investments & Dividends', 'chart.line.uptrend.xyaxis', '#3F51B5', 'INCOME'),
            ('cat_bonus', 'Thưởng & Quà tặng', 'Bonus & Gifts', 'gift.fill', '#E91E63', 'INCOME')
        ON CONFLICT (id) DO UPDATE SET name_en = EXCLUDED.name_en WHERE findora.categories.name_en IS NULL OR findora.categories.name_en = '';",
    )
    .execute(pool)
    .await?;

    // 5. Bảng Transactions
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS findora.transactions (
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
            sync_version BIGINT NOT NULL DEFAULT 1,
            created_at_tz TIMESTAMPTZ DEFAULT NOW()
        );
        ALTER TABLE findora.transactions ADD COLUMN IF NOT EXISTS created_at_tz TIMESTAMPTZ DEFAULT NOW();
        UPDATE findora.transactions SET created_at_tz = to_timestamp(timestamp / 1000.0) WHERE created_at_tz IS NULL;
        CREATE INDEX IF NOT EXISTS idx_tx_wallet_timestamp 
            ON findora.transactions(wallet_id, timestamp DESC) 
            WHERE deleted_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_tx_category 
            ON findora.transactions(category_id, timestamp DESC) 
            WHERE deleted_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_tx_sync_version 
            ON findora.transactions(wallet_id, sync_version);",
    )
    .execute(pool)
    .await?;

    // 6. Cấu hình Storage RLS Policies cho Supabase Storage Bucket 'findora_avatar_user'
    let _ = sqlx::raw_sql(
        "DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_policies WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'Public Access findora_avatar_user'
            ) THEN
                CREATE POLICY \"Public Access findora_avatar_user\"
                ON storage.objects FOR SELECT
                USING (bucket_id IN ('findora_avatar_user', 'findora-avatars'));
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_policies WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'Allow Upload findora_avatar_user'
            ) THEN
                CREATE POLICY \"Allow Upload findora_avatar_user\"
                ON storage.objects FOR INSERT
                WITH CHECK (bucket_id IN ('findora_avatar_user', 'findora-avatars'));
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_policies WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'Allow Update findora_avatar_user'
            ) THEN
                CREATE POLICY \"Allow Update findora_avatar_user\"
                ON storage.objects FOR UPDATE
                USING (bucket_id IN ('findora_avatar_user', 'findora-avatars'));
            END IF;
        END $$;"
    )
    .execute(pool)
    .await;

    // 7. Bảng Password Resets (Quên mật khẩu & Mã xác nhận 6 số)
    let _ = sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS findora.password_resets (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            code VARCHAR(10) NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            used_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_pwd_resets_email ON findora.password_resets(email, expires_at);
        CREATE INDEX IF NOT EXISTS idx_pwd_resets_email_used_at ON findora.password_resets(email, used_at);",
    )
    .execute(pool)
    .await;

    info!("🎯 Database schema Findora hoàn toàn sẵn sàng & chuẩn hóa.");
    Ok(())
}
