use dotenvy::dotenv;
use findora_backend::redis_client::RedisService;
use std::env;

#[tokio::test]
async fn test_redis_service_cache_and_invalidation() {
    dotenv().ok();
    dotenvy::from_filename("backend/.env").ok();
    dotenvy::from_filename(".env").ok();

    let redis_url = env::var("REDIS_URL").ok();
    let rest_url = env::var("UPSTASH_REDIS_REST_URL").ok();
    let rest_token = env::var("UPSTASH_REDIS_REST_TOKEN").ok();

    let redis = RedisService::new(
        redis_url.as_deref(),
        rest_url.as_deref(),
        rest_token.as_deref(),
    )
    .await;

    assert!(redis.is_enabled(), "Redis Service phải được kích hoạt");

    // 1. Test PING
    let is_alive = redis.ping().await;
    assert!(is_alive, "Redis ping phải thành công");

    // 2. Test SET & GET (Cache Hit)
    let key = "findora:test:category_cache";
    let data = r#"[{"id":"cat_food","name":"Ăn uống"}]"#;
    let ok = redis.set_ex(key, data, 60).await;
    assert!(ok, "SET key có TTL phải thành công");

    let cached = redis.get(key).await;
    assert_eq!(cached.as_deref(), Some(data), "GET key phải trả về đúng dữ liệu vừa lưu");

    // 3. Test DEL (Cache Invalidation)
    let del_ok = redis.del(key).await;
    assert!(del_ok, "DEL key phải thành công");

    let after_del = redis.get(key).await;
    assert_eq!(after_del, None, "Sau khi DEL thì GET phải trả về None");
}
