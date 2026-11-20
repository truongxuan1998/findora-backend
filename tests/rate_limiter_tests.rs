use findora_backend::rate_limiter::IpRateLimiter;

#[tokio::test]
async fn test_ip_rate_limiter_allow_and_block() {
    let limiter = IpRateLimiter::new();
    let ip = "192.168.1.100";
    let limit = 5;
    let window = 2; // 2 seconds

    // 1. Cho phép 5 requests đầu tiên
    for i in 1..=5 {
        let res = limiter.check(ip, limit, window).await;
        assert!(res.is_ok(), "Request {} phải được cho phép", i);
        let ok = res.unwrap();
        assert_eq!(ok.limit, 5);
        assert_eq!(ok.remaining, 5 - i);
    }

    // 2. Request thứ 6 vượt giới hạn -> Bị từ chối
    let res6 = limiter.check(ip, limit, window).await;
    assert!(res6.is_err(), "Request thứ 6 phải bị rate limit");
    let err = res6.unwrap_err();
    assert_eq!(err.limit, 5);
    assert!(err.retry_after_secs >= 1);

    // 3. IP khác không bị ảnh hưởng (cô lập theo từng IP)
    let other_ip = "192.168.1.200";
    let other_res = limiter.check(other_ip, limit, window).await;
    assert!(other_res.is_ok(), "IP khác phải có quota độc lập");
    assert_eq!(other_res.unwrap().remaining, 4);

    // 4. Chờ hết window 2 giây -> IP ban đầu được reset lại quota
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let res_reset = limiter.check(ip, limit, window).await;
    assert!(res_reset.is_ok(), "Sau khi hết window thì phải được reset quota");
    assert_eq!(res_reset.unwrap().remaining, 4);
}
