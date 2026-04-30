use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;

pub type AuthRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

pub fn create_login_limiter() -> AuthRateLimiter {
    // 10 requests per 5 minutes
    let quota =
        Quota::per_minute(NonZeroU32::new(2).unwrap()).allow_burst(NonZeroU32::new(10).unwrap());
    Arc::new(RateLimiter::direct(quota))
}

pub fn create_register_limiter() -> AuthRateLimiter {
    // 5 requests per 10 minutes
    let quota =
        Quota::per_minute(NonZeroU32::new(1).unwrap()).allow_burst(NonZeroU32::new(5).unwrap());
    Arc::new(RateLimiter::direct(quota))
}
