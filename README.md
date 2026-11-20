# Findora Backend (Rust + Axum + Supabase)

High-performance, memory-efficient backend service for the **Findora** personal finance app.

## 🚀 Features
- **Engine:** Rust (Axum, Tokio async runtime) — RAM footprint under 25MB.
- **Database:** Supabase PostgreSQL with dedicated `findora` schema.
- **Security:** Password hashing with `bcrypt`, Zero DB logging for security telemetry.
- **REST APIs:**
  - `GET /health` — Health check & Supabase connection test.
  - `POST /api/auth/register` — Register new user, create initial wallet.
  - `POST /api/auth/login` — Authenticate user.
  - `GET /api/categories` — List active categories.
  - `GET /api/reports/monthly` — Aggregated monthly balance & breakdown.

## 🌐 Live Production URL
- **Production API:** `https://findora-backend-zpr5.onrender.com`
- **Health Check:** `https://findora-backend-zpr5.onrender.com/health`
- **Region:** Singapore (Cloudflare CDN + Render Docker)

## 🛠 Local Development
```bash
# 1. Copy env
cp .env.example .env

# 2. Run
cargo run
```

## ☁️ Deploy to Render.com (Free Tier)
1. Push this repo to GitHub (e.g. `your-username/findora-backend`).
2. Go to [dashboard.render.com](https://dashboard.render.com) ➜ **New +** ➜ **Web Service**.
3. Select your GitHub repository `findora-backend`.
4. Choose **Docker** environment.
5. In **Environment Variables**, add:
   - `DATABASE_URL`: Your Supabase connection string.
6. Click **Deploy Web Service**!
